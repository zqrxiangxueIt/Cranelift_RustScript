use raii_demo::DynamicArray;

fn main() {
    println!("Initializing DynamicArray...");
    let mut array = DynamicArray::new();
    
    println!("Pushing elements...");
    for i in 0..10 {
        array.push(i);
    }
    
    assert_eq!(array.len(), 10);
    println!("Array length verified: {}", array.len());
    
    println!("Iterating elements:");
    for (i, val) in array.iter().enumerate() {
        println!("Index {}: {}", i, val);
        assert_eq!(i, *val);
    }
    
    println!("Popping elements...");
    while let Some(val) = array.pop() {
        print!("{} ", val);
    }
    println!("\nAll elements popped.");
    
    assert_eq!(array.len(), 0);
    println!("Integration test passed successfully.");
}
