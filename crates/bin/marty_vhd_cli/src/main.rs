use std::path::PathBuf;

use anyhow::{Result, bail};
use bpaf::Bpaf;
use marty_vhd::{Geometry, VhdBuilder, update};

/// Build a fixed-size FAT VHD from a directory.
#[derive(Debug, Bpaf)]
#[bpaf(options)]
struct Cli {
    /// Number of cylinders.
    #[bpaf(short, long)]
    cylinders: Option<u16>,

    /// Number of heads per cylinder.
    #[bpaf(short('H'), long)]
    heads: Option<u8>,

    /// Number of sectors per track.
    #[bpaf(short, long)]
    sectors: Option<u8>,

    /// Replace an existing VHD, reusing the geometry in its footer.
    #[bpaf(long, switch)]
    update: bool,

    /// FAT volume label (up to 11 characters).
    #[bpaf(long)]
    label: Option<String>,

    /// Directory whose contents become the root of the FAT filesystem.
    #[bpaf(positional("SOURCE"))]
    source: PathBuf,

    /// VHD file to create.
    #[bpaf(positional("OUTPUT"))]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = cli().run();
    if cli.update {
        if cli.cylinders.is_some() || cli.heads.is_some() || cli.sectors.is_some() {
            bail!("-c, -H, and -s cannot be used with --update");
        }
        return update(cli.source, cli.output, cli.label);
    }

    let (Some(cylinders), Some(heads), Some(sectors)) = (cli.cylinders, cli.heads, cli.sectors)
    else {
        bail!("-c, -H, and -s are required unless --update is specified");
    };

    let mut builder = VhdBuilder::new(cli.output, Geometry::new(cylinders, heads, sectors)?)
        .partitioned(true)
        .formatted(Some(cli.source));
    if let Some(label) = cli.label {
        builder = builder.with_label(label);
    }
    builder.build()
}
