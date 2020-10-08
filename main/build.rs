extern crate winrt;
use winrt::*;

fn main() {
    build!(
        dependencies
            os
        types
            windows::management::deployment::PackageManager
    );

    build();
}
