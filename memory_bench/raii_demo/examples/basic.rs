use raii_demo::DynamicArray;

fn main() {
    println!("--- Basic Usage Example ---");
    let mut arr = DynamicArray::new();
    
    // Push elements
    for i in 1..=5 {
        arr.push(i * 10);
        println!("Pushed: {}, len: {}, cap: {}", i * 10, arr.len(), arr.capacity());
    }

    // Iterate using Deref to slice
    println!("Elements: {:?}", &arr[..]);

    // Pop elements
    while let Some(val) = arr.pop() {
        println!("Popped: {}, len: {}", val, arr.len());
    }
}
