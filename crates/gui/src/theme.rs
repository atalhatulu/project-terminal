#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub background: (u8, u8, u8),
    pub foreground: (u8, u8, u8),
    pub cursor: [f32; 4],
    pub palette: [(u8, u8, u8); 16],
}

pub const TOKYO_NIGHT: Theme = Theme {
    name: "tokyo-night",
    background: (26, 27, 38), // #1a1b26
    foreground: (192, 202, 245), // #c0caf5
    cursor: [122.0 / 255.0, 162.0 / 255.0, 247.0 / 255.0, 0.9],
    palette: [
        (21, 22, 30),      // 0: Black (#15161e)
        (247, 118, 142),   // 1: Red (#f7768e)
        (158, 206, 106),   // 2: Green (#9ece6a)
        (224, 175, 104),   // 3: Yellow (#e0af68)
        (122, 162, 247),   // 4: Blue (#7aa2f7)
        (187, 154, 247),   // 5: Magenta (#bb9af7)
        (125, 207, 255),   // 6: Cyan (#7dcfff)
        (169, 177, 214),   // 7: White (#a9b1d6)
        (65, 72, 104),     // 8: Bright Black (#414868)
        (247, 118, 142),   // 9: Bright Red
        (115, 218, 139),   // 10: Bright Green (#73daca)
        (224, 175, 104),   // 11: Bright Yellow
        (122, 162, 247),   // 12: Bright Blue
        (187, 154, 247),   // 13: Bright Magenta
        (125, 207, 255),   // 14: Bright Cyan
        (192, 202, 245),   // 15: Bright White (#c0caf5)
    ],
};

pub const PURE_BLACK: Theme = Theme {
    name: "pure-black",
    background: (0, 0, 0),
    foreground: (226, 228, 234),
    cursor: [122.0 / 255.0, 162.0 / 255.0, 247.0 / 255.0, 0.9],
    palette: [
        (15, 15, 18),
        (247, 118, 142),
        (158, 206, 106),
        (224, 175, 104),
        (122, 162, 247),
        (187, 154, 247),
        (125, 207, 255),
        (226, 228, 234),
        (80, 85, 105),
        (255, 130, 155),
        (130, 225, 145),
        (235, 185, 115),
        (140, 180, 255),
        (200, 170, 255),
        (145, 220, 255),
        (255, 255, 255),
    ],
};

pub const CATPPUCCIN_MOCHA: Theme = Theme {
    name: "catppuccin-mocha",
    background: (30, 30, 46), // #1e1e2e
    foreground: (205, 214, 244), // #cdd6f4
    cursor: [245.0 / 255.0, 224.0 / 255.0, 220.0 / 255.0, 0.9], // #f5e0dc
    palette: [
        (69, 71, 90),     // 0: Surface1
        (243, 139, 168),  // 1: Red
        (166, 227, 161),  // 2: Green
        (249, 226, 175),  // 3: Yellow
        (137, 180, 250),  // 4: Blue
        (203, 166, 247),  // 5: Mauve
        (148, 226, 213),  // 6: Teal
        (186, 194, 222),  // 7: Subtext1
        (88, 91, 112),    // 8: Surface2
        (243, 139, 168),  // 9: Red
        (166, 227, 161),  // 10: Green
        (249, 226, 175),  // 11: Yellow
        (137, 180, 250),  // 12: Blue
        (203, 166, 247),  // 13: Mauve
        (148, 226, 213),  // 14: Teal
        (166, 173, 200),  // 15: Subtext0
    ],
};

pub const GRUVBOX_DARK: Theme = Theme {
    name: "gruvbox-dark",
    background: (40, 40, 40), // #282828
    foreground: (235, 219, 178), // #ebdbb2
    cursor: [254.0 / 255.0, 128.0 / 255.0, 25.0 / 255.0, 0.9], // #fe8019
    palette: [
        (40, 40, 40),     // 0: Dark 0
        (204, 36, 29),    // 1: Red
        (152, 151, 26),   // 2: Green
        (215, 153, 33),   // 3: Yellow
        (69, 133, 136),   // 4: Blue
        (177, 98, 134),   // 5: Purple
        (104, 157, 106),  // 6: Aqua
        (168, 153, 132),  // 7: Gray
        (146, 131, 116),  // 8: Gray
        (251, 73, 52),    // 9: Bright Red
        (184, 187, 38),   // 10: Bright Green
        (250, 189, 47),   // 11: Bright Yellow
        (131, 165, 152),  // 12: Bright Blue
        (211, 134, 155),  // 13: Bright Purple
        (142, 192, 124),  // 14: Bright Aqua
        (235, 219, 178),  // 15: Bright White
    ],
};

pub const ONE_DARK: Theme = Theme {
    name: "one-dark",
    background: (40, 44, 52), // #282c34
    foreground: (171, 178, 191), // #abb2bf
    cursor: [82.0 / 255.0, 139.0 / 255.0, 1.0, 0.9], // #528bff
    palette: [
        (30, 33, 39),     // 0: Black
        (224, 108, 117),  // 1: Red
        (152, 195, 121),  // 2: Green
        (209, 154, 102),  // 3: Yellow
        (97, 175, 239),   // 4: Blue
        (198, 120, 221),  // 5: Magenta
        (86, 182, 194),   // 6: Cyan
        (171, 178, 191),  // 7: White
        (92, 99, 112),    // 8: Light Black
        (224, 108, 117),  // 9: Bright Red
        (152, 195, 121),  // 10: Bright Green
        (209, 154, 102),  // 11: Bright Yellow
        (97, 175, 239),   // 12: Bright Blue
        (198, 120, 221),  // 13: Bright Magenta
        (86, 182, 194),   // 14: Bright Cyan
        (255, 255, 255),  // 15: Bright White
    ],
};

pub const DRACULA: Theme = Theme {
    name: "dracula",
    background: (40, 42, 54), // #282a36
    foreground: (248, 248, 242), // #f8f8f2
    cursor: [189.0 / 255.0, 147.0 / 255.0, 249.0 / 255.0, 0.9], // #bd93f9
    palette: [
        (33, 34, 44),     // 0: Background dark
        (255, 85, 85),    // 1: Red
        (80, 250, 123),   // 2: Green
        (241, 250, 140),  // 3: Yellow
        (189, 147, 249),  // 4: Purple
        (255, 121, 198),  // 5: Pink
        (139, 233, 253),  // 6: Cyan
        (248, 248, 242),  // 7: Foreground
        (98, 114, 164),   // 8: Comment
        (255, 110, 110),  // 9: Bright Red
        (105, 255, 148),  // 10: Bright Green
        (255, 255, 165),  // 11: Bright Yellow
        (214, 172, 255),  // 12: Bright Purple
        (255, 146, 223),  // 13: Bright Pink
        (164, 255, 255),  // 14: Bright Cyan
        (255, 255, 255),  // 15: White
    ],
};

pub const NORD: Theme = Theme {
    name: "nord",
    background: (46, 52, 64), // #2e3440
    foreground: (216, 222, 233), // #d8dee9
    cursor: [136.0 / 255.0, 192.0 / 255.0, 208.0 / 255.0, 0.9], // #88c0d0
    palette: [
        (59, 66, 82),     // 0: Polar Night
        (191, 97, 106),   // 1: Aurora Red
        (163, 190, 140),  // 2: Aurora Green
        (235, 203, 139),  // 3: Aurora Yellow
        (129, 161, 193),  // 4: Frost Blue
        (180, 142, 173),  // 5: Aurora Magenta
        (136, 192, 208),  // 6: Frost Cyan
        (229, 233, 240),  // 7: Snow Storm
        (76, 86, 106),    // 8: Polar Night Bright
        (191, 97, 106),   // 9: Bright Red
        (163, 190, 140),  // 10: Bright Green
        (235, 203, 139),  // 11: Bright Yellow
        (129, 161, 193),  // 12: Bright Blue
        (180, 142, 173),  // 13: Bright Magenta
        (143, 188, 187),  // 14: Frost Green-Cyan
        (236, 239, 244),  // 15: Bright Snow Storm
    ],
};

pub fn get_theme(name: &str) -> Option<&'static Theme> {
    match name.to_ascii_lowercase().replace('_', "-").as_str() {
        "tokyo-night" | "tokyonight" => Some(&TOKYO_NIGHT),
        "pure-black" | "black" | "pureblack" => Some(&PURE_BLACK),
        "catppuccin-mocha" | "catppuccin" | "mocha" => Some(&CATPPUCCIN_MOCHA),
        "gruvbox-dark" | "gruvbox" => Some(&GRUVBOX_DARK),
        "one-dark" | "onedark" => Some(&ONE_DARK),
        "dracula" => Some(&DRACULA),
        "nord" => Some(&NORD),
        _ => None,
    }
}
