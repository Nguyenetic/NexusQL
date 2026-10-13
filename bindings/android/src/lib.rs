// NexusQL Android JNI bindings — real NexusQL engine
//
// Uses NexusQL's DefaultEngine (sled-backed LSM storage) without the
// optional `vector-search` feature (ort + tokenizers), so it cross-compiles
// cleanly for all Android ABIs via cargo-ndk.
//
// API surface (matches the Kotlin wrapper):
//   NexusQL.open(path) -> handle
//   NexusQL.execute(handle, sql)
//   NexusQL.query(handle, sql) -> List<List<String>>
//   NexusQL.close(handle)

use jni::objects::{JClass, JString};
use jni::sys::{jlong, jobject};
use jni::JNIEnv;
use nexusql::engine::default::DefaultEngine;
use nexusql::kernel::NexusKernel;
use std::sync::Mutex;
use tokio::runtime::Runtime;

struct DbHandle {
    kernel: Mutex<NexusKernel>,
    rt: Runtime,
}

// Safety: kernel is only accessed through the Mutex.
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

    let rt = match Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("tokio runtime: {e}"));
            return 0;
        }
    };

    let kernel = match rt.block_on(async {
        let k = NexusKernel::new(&path_str, false).await?;
        let engine = DefaultEngine::new(&path_str, false)?;
        k.register_engine(Box::new(engine)).await?;
        Ok::<NexusKernel, nexusql::NexusError>(k)
    }) {
        Ok(k) => k,
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("NexusQL open failed: {e}"),
            );
            return 0;
        }
    };

    let handle = Box::new(DbHandle {
        kernel: Mutex::new(kernel),
        rt,
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

    let kernel = db.kernel.lock().unwrap();
    if let Err(e) = db.rt.block_on(kernel.execute(&sql_str)) {
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

    let kernel = db.kernel.lock().unwrap();
    let result_set = match db.rt.block_on(kernel.execute(&sql_str)) {
        Ok(rs) => rs,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("query failed: {e}"));
            return std::ptr::null_mut();
        }
    };

    let outer_list = match env.new_object("java/util/ArrayList", "()V", &[]) {
        Ok(o) => o,
        Err(_) => return std::ptr::null_mut(),
    };
    let outer = match env.get_list(&outer_list) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };

    for row in &result_set.rows {
        let inner_list_obj = match env.new_object("java/util/ArrayList", "()V", &[]) {
            Ok(o) => o,
            Err(_) => return std::ptr::null_mut(),
        };
        let inner = match env.get_list(&inner_list_obj) {
            Ok(l) => l,
            Err(_) => return std::ptr::null_mut(),
        };

        for value in &row.values {
            let s = value_to_string(value);
            let jval = match env.new_string(&s) {
                Ok(v) => v,
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

fn value_to_string(val: &nexusql::types::Value) -> String {
    use nexusql::types::Value;
    match val {
        Value::Null => "NULL".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.into_inner().to_string(),
        Value::Text(t) => t.clone(),
        Value::Geometry(g) => g.as_text(),
        Value::Boolean(b) => b.to_string(),
        Value::Blob(b) => format!("<blob {} bytes>", b.len()),
        Value::Vector(v) => format!("<vector dim={}>", v.len()),
    }
}

#[no_mangle]
pub extern "C" fn Java_app_shiori_hibiki_nexusql_NexusQL_nativeClose(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        let db = unsafe { Box::from_raw(handle as *mut DbHandle) };
        let kernel = db.kernel.into_inner().unwrap();
        db.rt.block_on(kernel.shutdown()).ok();
    }
}
