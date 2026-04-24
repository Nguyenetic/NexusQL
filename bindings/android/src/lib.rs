// NexusQL Android JNI bindings
//
// Exposes a SQLite-backed database with the same API surface as NexusQL:
//   NexusQL.open(path) -> handle
//   NexusQL.execute(handle, sql)
//   NexusQL.query(handle, sql) -> List<List<String>>
//   NexusQL.close(handle)
//
// Rationale for SQLite backend: NexusQL's main crate depends on `ort` (ONNX
// Runtime) and `tokenizers`, which cannot be cross-compiled for Android NDK
// in the current toolchain setup. This binding crate uses `rusqlite` (bundled
// SQLite) which compiles cleanly for all Android ABIs via cargo-ndk, and
// exposes the identical public API so the Kotlin caller is unchanged when the
// real NexusQL engine becomes Android-compatible.

use jni::objects::{JClass, JString};
use jni::sys::{jlong, jobject};
use jni::JNIEnv;
use rusqlite::Connection;
use std::sync::Mutex;

struct DbHandle {
    conn: Mutex<Connection>,
}

// Safety: DbHandle is only accessed through the Mutex, making it Send+Sync.
unsafe impl Send for DbHandle {}
unsafe impl Sync for DbHandle {}

#[no_mangle]
pub extern "C" fn Java_app_shiori_hibiki_nexusql_NexusQL_nativeOpen(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jlong {
    let path_str: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("bad path: {e}"));
            return 0;
        }
    };

    let conn = match Connection::open(&path_str) {
        Ok(c) => c,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("open failed: {e}"));
            return 0;
        }
    };

    let handle = Box::new(DbHandle {
        conn: Mutex::new(conn),
    });
    Box::into_raw(handle) as jlong
}

#[no_mangle]
pub extern "C" fn Java_app_shiori_hibiki_nexusql_NexusQL_nativeExecute(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    sql: JString,
) {
    if handle == 0 {
        let _ = env.throw_new("java/lang/RuntimeException", "null database handle");
        return;
    }
    let db = unsafe { &*(handle as *const DbHandle) };
    let sql_str: String = match env.get_string(&sql) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("bad sql: {e}"));
            return;
        }
    };

    let conn = db.conn.lock().unwrap();
    if let Err(e) = conn.execute_batch(&sql_str) {
        let _ = env.throw_new("java/lang/RuntimeException", format!("execute failed: {e}"));
    }
}

#[no_mangle]
pub extern "C" fn Java_app_shiori_hibiki_nexusql_NexusQL_nativeQuery(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    sql: JString,
) -> jobject {
    if handle == 0 {
        let _ = env.throw_new("java/lang/RuntimeException", "null database handle");
        return std::ptr::null_mut();
    }

    let db = unsafe { &*(handle as *const DbHandle) };
    let sql_str: String = match env.get_string(&sql) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("bad sql: {e}"));
            return std::ptr::null_mut();
        }
    };

    let conn = db.conn.lock().unwrap();
    let outer_list = match env.new_object("java/util/ArrayList", "()V", &[]) {
        Ok(o) => o,
        Err(_) => return std::ptr::null_mut(),
    };
    let outer = match env.get_list(&outer_list) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };

    let mut stmt = match conn.prepare(&sql_str) {
        Ok(s) => s,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("prepare failed: {e}"));
            return std::ptr::null_mut();
        }
    };

    let col_count = stmt.column_count();
    let mut rows = match stmt.query([]) {
        Ok(r) => r,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("query failed: {e}"));
            return std::ptr::null_mut();
        }
    };

    loop {
        let row = match rows.next() {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => {
                let _ = env.throw_new("java/lang/RuntimeException", format!("row error: {e}"));
                return std::ptr::null_mut();
            }
        };

        let inner_list_obj = match env.new_object("java/util/ArrayList", "()V", &[]) {
            Ok(o) => o,
            Err(_) => return std::ptr::null_mut(),
        };
        let inner = match env.get_list(&inner_list_obj) {
            Ok(l) => l,
            Err(_) => return std::ptr::null_mut(),
        };

        for col in 0..col_count {
            let val = match row.get_ref(col) {
                Ok(v) => value_to_string(v),
                Err(_) => "NULL".to_string(),
            };
            let jval = match env.new_string(&val) {
                Ok(s) => s,
                Err(_) => return std::ptr::null_mut(),
            };
            if inner.add(&mut env, &jval).is_err() {
                return std::ptr::null_mut();
            }
        }

        if outer.add(&mut env, &inner_list_obj).is_err() {
            return std::ptr::null_mut();
        }
    }

    outer_list.into_raw()
}

#[no_mangle]
pub extern "C" fn Java_app_shiori_hibiki_nexusql_NexusQL_nativeClose(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        // Reconstruct the Box and drop it, closing the connection.
        let _ = unsafe { Box::from_raw(handle as *mut DbHandle) };
    }
}

fn value_to_string(val: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match val {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}
