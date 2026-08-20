use crate::device_types::{geometry::DriveGeometry, hdc::HardDiskFormat};

pub struct AtFormats {}

impl AtFormats {
    #[rustfmt::skip]
    pub fn vec() -> Vec<HardDiskFormat> {
        vec![
            // 0 - 7
            HardDiskFormat { geometry: DriveGeometry::new(306, 4, 17, 1, 512), wpc: None, desc: "10MB (306/4/17)".to_string()},
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 2, 17, 1, 512),
                wpc: None,
                desc: "10MB (615/2/17)".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(306, 4, 26, 1, 512),
                wpc: None,
                desc: "15MB (306/4/26)".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 2, 17, 1, 512),
                wpc: None,
                desc: "17MB (1024/2/17)".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(697, 3, 17, 1, 512),
                wpc: None,
                desc: "17MB (697/3/17)".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(306, 8, 17, 1, 512),
                wpc: None,
                desc: "20MB (306/8/17)".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(614, 4, 17, 1, 512),
                wpc: None,
                desc: "20MB (614/4/17)".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 4, 17, 1, 512),
                wpc: None,
                desc: "20MB 615/4/17".to_string(),
            },
            // 8 - 15
            HardDiskFormat {
                geometry: DriveGeometry::new(670, 4, 17, 1, 512),
                wpc: None,
                desc: "22MB 670/4/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(697, 4, 17, 1, 512),
                wpc: None,
                desc: "23MB 697/4/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(987, 3, 17, 1, 512),
                wpc: None,
                desc: "24MB 987/3/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(820, 4, 17, 1, 512),
                wpc: None,
                desc: "27MB 820/4/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(670, 5, 17, 1, 512),
                wpc: None,
                desc: "27MB 670/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(697, 5, 17, 1, 512),
                wpc: None,
                desc: "28MB 697/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(733, 5, 17, 1, 512),
                wpc: None,
                desc: "30MB 733/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 6, 17, 1, 512),
                wpc: None,
                desc: "30MB 615/6/17".to_string(),
            },
            // 16 - 23
            HardDiskFormat {
                geometry: DriveGeometry::new(462, 8, 17, 1, 512),
                wpc: None,
                desc: "30MB 462/8/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(306, 8, 26, 1, 512),
                wpc: None,
                desc: "31MB 306/8/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 4, 26, 1, 512),
                wpc: None,
                desc: "31MB 615/4/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 4, 17, 1, 512),
                wpc: None,
                desc: "34MB 1024/4/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(855, 5, 17, 1, 512),
                wpc: None,
                desc: "35MB 855/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(925, 5, 17, 1, 512),
                wpc: None,
                desc: "925/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(932, 5, 17, 1, 512),
                wpc: None,
                desc: "932/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 2, 40, 1, 512),
                wpc: None,
                desc: "1024/2/40".to_string(),
            },
            // 24 - 31
            HardDiskFormat {
                geometry: DriveGeometry::new(809, 6, 17, 1, 512),
                wpc: None,
                desc: "809/6/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(976, 5, 17, 1, 512),
                wpc: None,
                desc: "976/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(977, 5, 17, 1, 512),
                wpc: None,
                desc: "977/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(698, 7, 17, 1, 512),
                wpc: None,
                desc: "698/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(699, 7, 17, 1, 512),
                wpc: None,
                desc: "699/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(981, 5, 17, 1, 512),
                wpc: None,
                desc: "981/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 8, 17, 1, 512),
                wpc: None,
                desc: "615/8/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(989, 5, 17, 1, 512),
                wpc: None,
                desc: "989/5/17".to_string(),
            },
            // 32 - 39
            HardDiskFormat {
                geometry: DriveGeometry::new(820, 4, 26, 1, 512),
                wpc: None,
                desc: "820/4/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 5, 17, 1, 512),
                wpc: None,
                desc: "1024/5/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(733, 7, 17, 1, 512),
                wpc: None,
                desc: "733/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(754, 7, 17, 1, 512),
                wpc: None,
                desc: "754/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(733, 5, 26, 1, 512),
                wpc: None,
                desc: "733/5/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(940, 6, 17, 1, 512),
                wpc: None,
                desc: "940/6/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 6, 26, 1, 512),
                wpc: None,
                desc: "615/6/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(462, 8, 26, 1, 512),
                wpc: None,
                desc: "462/8/26".to_string(),
            },
            // 40 - 47
            HardDiskFormat {
                geometry: DriveGeometry::new(830, 7, 17, 1, 512),
                wpc: None,
                desc: "830/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(855, 7, 17, 1, 512),
                wpc: None,
                desc: "855/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(751, 8, 17, 1, 512),
                wpc: None,
                desc: "751/8/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 4, 26, 1, 512),
                wpc: None,
                desc: "1024/4/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(918, 7, 17, 1, 512),
                wpc: None,
                desc: "918/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(925, 7, 17, 1, 512),
                wpc: None,
                desc: "925/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(855, 5, 26, 1, 512),
                wpc: None,
                desc: "855/5/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(977, 7, 17, 1, 512),
                wpc: None,
                desc: "977/7/17".to_string(),
            },
            // 48 - 55
            HardDiskFormat {
                geometry: DriveGeometry::new(987, 7, 17, 1, 512),
                wpc: None,
                desc: "987/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 7, 17, 1, 512),
                wpc: None,
                desc: "1024/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(823, 4, 38, 1, 512),
                wpc: None,
                desc: "823/4/38".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(925, 8, 17, 1, 512),
                wpc: None,
                desc: "925/8/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(809, 6, 26, 1, 512),
                wpc: None,
                desc: "809/6/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(976, 5, 26, 1, 512),
                wpc: None,
                desc: "976/5/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(977, 5, 26, 1, 512),
                wpc: None,
                desc: "977/5/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(698, 7, 26, 1, 512),
                wpc: None,
                desc: "698/7/26".to_string(),
            },
            // 56 - 63
            HardDiskFormat {
                geometry: DriveGeometry::new(699, 7, 26, 1, 512),
                wpc: None,
                desc: "699/7/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(940, 8, 17, 1, 512),
                wpc: None,
                desc: "940/8/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(615, 8, 26, 1, 512),
                wpc: None,
                desc: "615/8/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 5, 26, 1, 512),
                wpc: None,
                desc: "1024/5/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(733, 7, 26, 1, 512),
                wpc: None,
                desc: "733/7/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 8, 17, 1, 512),
                wpc: None,
                desc: "1024/8/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(823, 10, 17, 1, 512),
                wpc: None,
                desc: "823/10/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(754, 11, 17, 1, 512),
                wpc: None,
                desc: "754/11/17".to_string(),
            },
            // 64 - 71
            HardDiskFormat {
                geometry: DriveGeometry::new(830, 10, 17, 1, 512),
                wpc: None,
                desc: "830/10/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(925, 9, 17, 1, 512),
                wpc: None,
                desc: "925/9/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1224, 7, 17, 1, 512),
                wpc: None,
                desc: "1224/7/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(940, 6, 26, 1, 512),
                wpc: None,
                desc: "940/6/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(855, 7, 26, 1, 512),
                wpc: None,
                desc: "855/7/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(751, 8, 26, 1, 512),
                wpc: None,
                desc: "751/8/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 9, 17, 1, 512),
                wpc: None,
                desc: "1024/9/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(965, 10, 17, 1, 512),
                wpc: None,
                desc: "965/10/17".to_string(),
            },
            // 72 - 79
            HardDiskFormat {
                geometry: DriveGeometry::new(969, 5, 34, 1, 512),
                wpc: None,
                desc: "969/5/34".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(980, 10, 17, 1, 512),
                wpc: None,
                desc: "980/10/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(960, 5, 35, 1, 512),
                wpc: None,
                desc: "960/5/35".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(918, 11, 17, 1, 512),
                wpc: None,
                desc: "918/11/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 10, 17, 1, 512),
                wpc: None,
                desc: "1024/10/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(977, 7, 26, 1, 512),
                wpc: None,
                desc: "977/7/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 7, 26, 1, 512),
                wpc: None,
                desc: "1024/7/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 11, 17, 1, 512),
                wpc: None,
                desc: "1024/11/17".to_string(),
            },
            // 80 - 87
            HardDiskFormat {
                geometry: DriveGeometry::new(940, 8, 26, 1, 512),
                wpc: None,
                desc: "940/8/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(776, 8, 33, 1, 512),
                wpc: None,
                desc: "776/8/33".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(755, 16, 17, 1, 512),
                wpc: None,
                desc: "755/16/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 12, 17, 1, 512),
                wpc: None,
                desc: "1024/12/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 8, 26, 1, 512),
                wpc: None,
                desc: "1024/8/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(823, 10, 26, 1, 512),
                wpc: None,
                desc: "823/10/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(830, 10, 26, 1, 512),
                wpc: None,
                desc: "830/10/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(925, 9, 26, 1, 512),
                wpc: None,
                desc: "925/9/26".to_string(),
            },
            // 88 - 95
            HardDiskFormat {
                geometry: DriveGeometry::new(960, 9, 26, 1, 512),
                wpc: None,
                desc: "960/9/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 13, 17, 1, 512),
                wpc: None,
                desc: "1024/13/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1224, 11, 17, 1, 512),
                wpc: None,
                desc: "1224/11/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(900, 15, 17, 1, 512),
                wpc: None,
                desc: "900/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(969, 7, 34, 1, 512),
                wpc: None,
                desc: "969/7/34".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(917, 15, 17, 1, 512),
                wpc: None,
                desc: "917/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(918, 15, 17, 1, 512),
                wpc: None,
                desc: "918/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1524, 4, 39, 1, 512),
                wpc: None,
                desc: "1524/4/39".to_string(),
            },
            // 96 - 103
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 9, 26, 1, 512),
                wpc: None,
                desc: "1024/9/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 14, 17, 1, 512),
                wpc: None,
                desc: "1024/14/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(965, 10, 26, 1, 512),
                wpc: None,
                desc: "965/10/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(980, 10, 26, 1, 512),
                wpc: None,
                desc: "980/10/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1020, 15, 17, 1, 512),
                wpc: None,
                desc: "1020/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1023, 15, 17, 1, 512),
                wpc: None,
                desc: "1023/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 15, 17, 1, 512),
                wpc: None,
                desc: "1024/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 16, 17, 1, 512),
                wpc: None,
                desc: "1024/16/17".to_string(),
            },
            // 104 - 111
            HardDiskFormat {
                geometry: DriveGeometry::new(1224, 15, 17, 1, 512),
                wpc: None,
                desc: "1224/15/17".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(755, 16, 26, 1, 512),
                wpc: None,
                desc: "755/16/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(903, 8, 46, 1, 512),
                wpc: None,
                desc: "903/8/46".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(984, 10, 34, 1, 512),
                wpc: None,
                desc: "984/10/34".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(900, 15, 26, 1, 512),
                wpc: None,
                desc: "900/15/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(917, 15, 26, 1, 512),
                wpc: None,
                desc: "917/15/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1023, 15, 26, 1, 512),
                wpc: None,
                desc: "1023/15/26".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(684, 16, 38, 1, 512),
                wpc: None,
                desc: "684/16/38".to_string(),
            },
            // 112 - 119 (note the overlapping index 119 below—data as provided)
            HardDiskFormat {
                geometry: DriveGeometry::new(1930, 4, 62, 1, 512),
                wpc: None,
                desc: "1930/4/62".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(967, 16, 31, 1, 512),
                wpc: None,
                desc: "967/16/31".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1013, 10, 63, 1, 512),
                wpc: None,
                desc: "1013/10/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1218, 15, 36, 1, 512),
                wpc: None,
                desc: "1218/15/36".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(654, 16, 63, 1, 512),
                wpc: None,
                desc: "654/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(659, 16, 63, 1, 512),
                wpc: None,
                desc: "659/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(702, 16, 63, 1, 512),
                wpc: None,
                desc: "702/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1002, 13, 63, 1, 512),
                wpc: None,
                desc: "1002/13/63".to_string(),
            },
            // Overlapping "119" again - 119 - 125
            HardDiskFormat {
                geometry: DriveGeometry::new(854, 16, 63, 1, 512),
                wpc: None,
                desc: "854/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(987, 16, 63, 1, 512),
                wpc: None,
                desc: "987/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(995, 16, 63, 1, 512),
                wpc: None,
                desc: "995/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1024, 16, 63, 1, 512),
                wpc: None,
                desc: "1024/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1036, 16, 63, 1, 512),
                wpc: None,
                desc: "1036/16/63".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1120, 16, 59, 1, 512),
                wpc: None,
                desc: "1120/16/59".to_string(),
            },
            HardDiskFormat {
                geometry: DriveGeometry::new(1054, 16, 63, 1, 512),
                wpc: None,
                desc: "1054/16/63".to_string(),
            },
        ]
    }
}
