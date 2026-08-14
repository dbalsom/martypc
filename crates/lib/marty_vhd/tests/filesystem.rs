#![cfg(feature = "builder")]

use std::{fs, path::PathBuf};

use marty_vhd::{Geometry, VhdBuilder};

#[test]
fn builds_test_filesystem_as_test_vhd() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = crate_root.join("tests/filesystem");
    let output = crate_root.join("tests/TEST.VHD");

    assert!(source.is_dir(), "missing source directory: {}", source.display());
    if output.is_file() {
        fs::remove_file(&output).expect("could not replace tests/TEST.VHD");
    }

    VhdBuilder::new(
        output.clone(),
        Geometry::new(306, 4, 17).expect("invalid test geometry"),
    )
    .partitioned(true)
    .formatted(Some(source))
    .with_label("TEST_VHD")
    .build()
    .expect("could not build tests/TEST.VHD");

    assert!(output.is_file(), "build did not create {}", output.display());
}
