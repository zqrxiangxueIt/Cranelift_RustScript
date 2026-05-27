use raii_demo::DynamicArray;
use std::thread;

fn main() {
    println!("--- Cross-thread Ownership Transfer Example ---");
    let mut arr = DynamicArray::new();
    for i in 0..10 {
        arr.push(i);
    }

    println!("Original array (thread main): {:?}", &arr[..]);

    // Move ownership to another thread
    let handle = thread::spawn(move || {
        println!("Array in new thread: {:?}", &arr[..]);
        arr.push(100);
        arr // Return ownership back
    });

    let arr = handle.join().unwrap();
    println!("Array back in main thread: {:?}", &arr[..]);
}
