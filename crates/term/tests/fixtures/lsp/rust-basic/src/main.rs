/// A well-documented function for hover testing.
///
/// # Arguments
/// * `x` - The input value
///
/// # Returns
/// The input plus one
fn documented_function(x: i32) -> i32 {
    x + 1
}

fn main() {
    // Unused variable - should produce warning diagnostic
    let unused_var = 42;

    // Type error - should produce error diagnostic
    let type_error: String = 123;

    // Call documented function
    let result = documented_function(5);
    println!("{}", result);
}
