// Week 16: JSON/JSONB & Arrays Demo
// Demonstrates JSON/JSONB operators and array operations

use nexusql::jsonb::{ArrayOps, JsonbFunctions, JsonbValue};
use nexusql::types::Value;

fn main() {
    println!("🚀 NexusQL Week 16: JSON/JSONB & Arrays Demo");
    println!("============================================\n");

    demo_jsonb_basics();
    demo_jsonb_operators();
    demo_jsonb_path_extraction();
    demo_jsonb_containment();
    demo_jsonb_functions();
    demo_array_operations();

    println!("\n✅ All demos completed successfully!");
}

fn demo_jsonb_basics() {
    println!("📦 Demo 1: JSONB Basics");
    println!("----------------------\n");

    let user = JsonbValue::from_str(
        r#"{
        "id": 1,
        "name": "Alice",
        "email": "alice@example.com",
        "age": 30,
        "active": true
    }"#,
    )
    .unwrap();

    println!("User JSONB document:");
    println!("{}\n", user.to_string());

    // Extract fields
    let name = user.extract_field_text("name").unwrap();
    let age = user.extract_field("age").unwrap();

    println!("Extracted fields:");
    println!("  name: {}", name);
    println!("  age: {}", age.to_string());
}

fn demo_jsonb_operators() {
    println!("\n\n🔧 Demo 2: JSONB Operators");
    println!("--------------------------\n");

    let product = JsonbValue::from_str(
        r#"{
        "id": 101,
        "name": "Laptop",
        "tags": ["electronics", "computers"],
        "specs": {
            "cpu": "Intel i7",
            "ram": "16GB"
        }
    }"#,
    )
    .unwrap();

    // -> operator (extract field)
    println!("-> operator (extract field):");
    let specs = product.extract_field("specs").unwrap();
    println!("  specs: {}\n", specs.to_string());

    // ->> operator (extract as text)
    println!("->> operator (extract as text):");
    let name = product.extract_field_text("name").unwrap();
    println!("  name: {}\n", name);

    // ? operator (key exists)
    println!("? operator (key exists):");
    println!("  'name' exists: {}", product.has_key("name"));
    println!("  'price' exists: {}\n", product.has_key("price"));

    // Delete key
    println!("- operator (delete key):");
    let without_id = product.delete_key("id");
    println!("  After deleting 'id': {}", without_id.has_key("id"));
}

fn demo_jsonb_path_extraction() {
    println!("\n\n🗺️  Demo 3: JSONB Path Extraction (#> operator)");
    println!("-----------------------------------------------\n");

    let data = JsonbValue::from_str(
        r#"{
        "company": {
            "name": "TechCorp",
            "employees": [
                {"name": "Alice", "role": "Engineer"},
                {"name": "Bob", "role": "Manager"}
            ]
        }
    }"#,
    )
    .unwrap();

    // Extract nested path
    let path1 = vec!["company".to_string(), "name".to_string()];
    let company_name = data.extract_path(&path1).unwrap();
    println!("Path: $.company.name");
    println!("  Result: {}\n", company_name.to_string());

    let path2 = vec![
        "company".to_string(),
        "employees".to_string(),
        "0".to_string(),
        "name".to_string(),
    ];
    let first_employee = data.extract_path(&path2).unwrap();
    println!("Path: $.company.employees[0].name");
    println!("  Result: {}", first_employee.to_string());
}

fn demo_jsonb_containment() {
    println!("\n\n🎯 Demo 4: JSONB Containment (@>, <@)");
    println!("-------------------------------------\n");

    let full_doc = JsonbValue::from_str(
        r#"{
        "name": "Alice",
        "age": 30,
        "city": "NYC",
        "tags": ["developer", "rust"]
    }"#,
    )
    .unwrap();

    let subset1 = JsonbValue::from_str(r#"{"name": "Alice"}"#).unwrap();
    let subset2 = JsonbValue::from_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
    let not_subset = JsonbValue::from_str(r#"{"name": "Bob"}"#).unwrap();

    println!("Full document contains:");
    println!(
        "  {}: {}",
        r#"{"name": "Alice"}"#,
        full_doc.contains(&subset1)
    );
    println!(
        "  {}: {}",
        r#"{"name": "Alice", "age": 30}"#,
        full_doc.contains(&subset2)
    );
    println!(
        "  {}: {}\n",
        r#"{"name": "Bob"}"#,
        full_doc.contains(&not_subset)
    );

    println!("Use case: Find users with specific attributes");
    println!("  WHERE user_data @> '{{\"city\": \"NYC\"}}'");
}

fn demo_jsonb_functions() {
    println!("\n\n🔨 Demo 5: JSONB Functions");
    println!("--------------------------\n");

    // jsonb_build_object
    println!("jsonb_build_object():");
    let obj = JsonbFunctions::build_object(vec![
        ("product".to_string(), Value::Text("Widget".to_string())),
        ("price".to_string(), Value::Integer(999)),
        ("in_stock".to_string(), Value::Boolean(true)),
    ])
    .unwrap();
    println!("  Result: {}\n", obj.to_string());

    // jsonb_build_array
    println!("jsonb_build_array():");
    let arr = JsonbFunctions::build_array(vec![
        Value::Integer(1),
        Value::Integer(2),
        Value::Integer(3),
    ])
    .unwrap();
    println!("  Result: {}\n", arr.to_string());

    // jsonb_agg
    println!("jsonb_agg() (aggregate):");
    let values = vec![
        Value::Text("apple".to_string()),
        Value::Text("banana".to_string()),
        Value::Text("cherry".to_string()),
    ];
    let agg = JsonbFunctions::agg(values).unwrap();
    println!("  Result: {}", agg.to_string());
}

fn demo_array_operations() {
    println!("\n\n📊 Demo 6: Array Operations");
    println!("---------------------------\n");

    let array1 = vec![
        Value::Text("apple".to_string()),
        Value::Text("banana".to_string()),
        Value::Text("cherry".to_string()),
    ];

    let array2 = vec![
        Value::Text("banana".to_string()),
        Value::Text("date".to_string()),
    ];

    // Array length
    println!(
        "Array 1: {:?}",
        array1.iter().map(|v| v.to_string()).collect::<Vec<_>>()
    );
    println!("Length: {}\n", ArrayOps::length(&array1));

    // Contains
    println!("@> operator (contains):");
    println!(
        "  Array contains 'banana': {}",
        ArrayOps::contains(&array1, &Value::Text("banana".to_string()))
    );
    println!(
        "  Array contains 'grape': {}\n",
        ArrayOps::contains(&array1, &Value::Text("grape".to_string()))
    );

    // Overlaps
    println!("&& operator (overlaps):");
    println!(
        "  Array 1 overlaps Array 2: {}\n",
        ArrayOps::overlaps(&array1, &array2)
    );

    // Concatenation
    println!("|| operator (concatenation):");
    let concat = ArrayOps::concat(array1.clone(), array2.clone());
    println!(
        "  Result: {:?}",
        concat.iter().map(|v| v.to_string()).collect::<Vec<_>>()
    );
    println!("  Length: {}\n", ArrayOps::length(&concat));

    // Unnest
    println!("unnest() function:");
    let unnested = ArrayOps::unnest(array1);
    println!("  Expanded {} elements:", unnested.len());
    for (i, val) in unnested.iter().enumerate() {
        println!("    Row {}: {}", i + 1, val);
    }

    // array_agg
    println!("\narray_agg() (aggregate):");
    let values = vec![Value::Integer(10), Value::Integer(20), Value::Integer(30)];
    let agg = ArrayOps::agg(values);
    println!("  Aggregated {} values into array", agg.len());
}
