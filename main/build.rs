extern crate winrt;

fn main() {
    winrt::build!(
        dependencies
            os
        types
            windows::management::deployment::PackageManager
    );

    build();
}
