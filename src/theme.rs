//! Colour palettes. Every colour the UI uses comes from here, so a theme can
//! be swapped at runtime and there is exactly one place to look when something
//! is too dim.

use ratatui::style::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub fg: Color,
    pub muted: Color,
    pub dim: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub accent_fg: Color,
    pub border: Color,
    pub border_focus: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub connected: Color,
    pub paired: Color,
    pub trusted: Color,
    pub blocked: Color,
    pub gauge_empty: Color,
    pub chart: Vec<Color>,
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    /// The built-in catalogue, in the order shown by the theme picker.
    pub fn catalogue() -> Vec<Theme> {
        vec![
            Theme::midnight(),
            Theme::nord(),
            Theme::catppuccin(),
            Theme::dracula(),
            Theme::gruvbox(),
            Theme::tokyo_night(),
            Theme::rose_pine(),
            Theme::solarized(),
            Theme::light(),
        ]
    }

    pub fn by_name(name: &str) -> Option<Theme> {
        Theme::catalogue()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    pub fn name_list() -> Vec<&'static str> {
        Theme::catalogue().into_iter().map(|t| t.name).collect()
    }

    pub fn midnight() -> Theme {
        Theme {
            name: "Midnight",
            bg: rgb(13, 17, 27),
            surface: rgb(20, 26, 40),
            surface_alt: rgb(26, 33, 50),
            fg: rgb(220, 228, 242),
            muted: rgb(140, 155, 180),
            dim: rgb(74, 87, 110),
            accent: rgb(94, 196, 255),
            accent_soft: rgb(40, 90, 130),
            accent_fg: rgb(8, 15, 26),
            border: rgb(52, 66, 90),
            border_focus: rgb(94, 196, 255),
            header_bg: rgb(20, 32, 52),
            header_fg: rgb(224, 236, 250),
            selection_bg: rgb(38, 84, 124),
            selection_fg: rgb(238, 248, 255),
            success: rgb(94, 220, 160),
            warning: rgb(245, 190, 90),
            error: rgb(245, 105, 110),
            info: rgb(120, 180, 255),
            connected: rgb(94, 220, 160),
            paired: rgb(150, 170, 255),
            trusted: rgb(190, 150, 255),
            blocked: rgb(245, 105, 110),
            gauge_empty: rgb(38, 48, 66),
            chart: vec![
                rgb(94, 196, 255),
                rgb(94, 220, 160),
                rgb(245, 190, 90),
                rgb(245, 105, 110),
            ],
        }
    }

    fn nord() -> Theme {
        Theme {
            name: "Nord",
            bg: rgb(46, 52, 64),
            surface: rgb(59, 66, 82),
            surface_alt: rgb(67, 76, 94),
            fg: rgb(236, 239, 244),
            muted: rgb(180, 190, 205),
            dim: rgb(118, 131, 150),
            accent: rgb(136, 192, 208),
            accent_soft: rgb(76, 106, 120),
            accent_fg: rgb(46, 52, 64),
            border: rgb(76, 86, 106),
            border_focus: rgb(136, 192, 208),
            header_bg: rgb(59, 66, 82),
            header_fg: rgb(236, 239, 244),
            selection_bg: rgb(94, 129, 152),
            selection_fg: rgb(255, 255, 255),
            success: rgb(163, 190, 140),
            warning: rgb(235, 203, 139),
            error: rgb(191, 97, 106),
            info: rgb(129, 161, 193),
            connected: rgb(163, 190, 140),
            paired: rgb(143, 188, 187),
            trusted: rgb(180, 142, 173),
            blocked: rgb(191, 97, 106),
            gauge_empty: rgb(67, 76, 94),
            chart: vec![
                rgb(136, 192, 208),
                rgb(163, 190, 140),
                rgb(235, 203, 139),
                rgb(191, 97, 106),
            ],
        }
    }

    fn catppuccin() -> Theme {
        Theme {
            name: "Catppuccin",
            bg: rgb(24, 24, 37),
            surface: rgb(30, 30, 46),
            surface_alt: rgb(39, 39, 58),
            fg: rgb(205, 214, 244),
            muted: rgb(166, 173, 200),
            dim: rgb(88, 91, 112),
            accent: rgb(137, 180, 250),
            accent_soft: rgb(69, 95, 133),
            accent_fg: rgb(17, 17, 27),
            border: rgb(69, 71, 90),
            border_focus: rgb(137, 180, 250),
            header_bg: rgb(30, 30, 46),
            header_fg: rgb(205, 214, 244),
            selection_bg: rgb(69, 95, 133),
            selection_fg: rgb(255, 255, 255),
            success: rgb(166, 227, 161),
            warning: rgb(249, 226, 175),
            error: rgb(243, 139, 168),
            info: rgb(116, 199, 236),
            connected: rgb(166, 227, 161),
            paired: rgb(137, 180, 250),
            trusted: rgb(203, 166, 247),
            blocked: rgb(243, 139, 168),
            gauge_empty: rgb(49, 50, 68),
            chart: vec![
                rgb(137, 180, 250),
                rgb(166, 227, 161),
                rgb(249, 226, 175),
                rgb(243, 139, 168),
            ],
        }
    }

    fn dracula() -> Theme {
        Theme {
            name: "Dracula",
            bg: rgb(40, 42, 54),
            surface: rgb(48, 50, 64),
            surface_alt: rgb(58, 60, 76),
            fg: rgb(248, 248, 242),
            muted: rgb(189, 191, 205),
            dim: rgb(98, 100, 120),
            accent: rgb(189, 147, 249),
            accent_soft: rgb(95, 75, 132),
            accent_fg: rgb(24, 25, 34),
            border: rgb(68, 71, 90),
            border_focus: rgb(189, 147, 249),
            header_bg: rgb(48, 50, 64),
            header_fg: rgb(248, 248, 242),
            selection_bg: rgb(68, 51, 101),
            selection_fg: rgb(255, 255, 255),
            success: rgb(80, 250, 123),
            warning: rgb(241, 250, 140),
            error: rgb(255, 85, 85),
            info: rgb(139, 233, 253),
            connected: rgb(80, 250, 123),
            paired: rgb(139, 233, 253),
            trusted: rgb(255, 121, 198),
            blocked: rgb(255, 85, 85),
            gauge_empty: rgb(68, 71, 90),
            chart: vec![
                rgb(189, 147, 249),
                rgb(80, 250, 123),
                rgb(241, 250, 140),
                rgb(255, 85, 85),
            ],
        }
    }

    fn gruvbox() -> Theme {
        Theme {
            name: "Gruvbox",
            bg: rgb(40, 40, 40),
            surface: rgb(50, 48, 47),
            surface_alt: rgb(60, 56, 54),
            fg: rgb(235, 219, 178),
            muted: rgb(189, 174, 147),
            dim: rgb(124, 111, 100),
            accent: rgb(250, 189, 47),
            accent_soft: rgb(124, 94, 27),
            accent_fg: rgb(40, 40, 40),
            border: rgb(80, 73, 69),
            border_focus: rgb(250, 189, 47),
            header_bg: rgb(60, 56, 54),
            header_fg: rgb(235, 219, 178),
            selection_bg: rgb(102, 92, 68),
            selection_fg: rgb(251, 241, 199),
            success: rgb(184, 187, 38),
            warning: rgb(250, 189, 47),
            error: rgb(251, 73, 52),
            info: rgb(131, 165, 152),
            connected: rgb(184, 187, 38),
            paired: rgb(131, 165, 152),
            trusted: rgb(211, 134, 155),
            blocked: rgb(251, 73, 52),
            gauge_empty: rgb(60, 56, 54),
            chart: vec![
                rgb(250, 189, 47),
                rgb(184, 187, 38),
                rgb(254, 128, 25),
                rgb(251, 73, 52),
            ],
        }
    }

    fn tokyo_night() -> Theme {
        Theme {
            name: "Tokyo Night",
            bg: rgb(26, 27, 38),
            surface: rgb(32, 34, 48),
            surface_alt: rgb(41, 44, 60),
            fg: rgb(192, 202, 245),
            muted: rgb(140, 150, 190),
            dim: rgb(72, 79, 110),
            accent: rgb(122, 162, 247),
            accent_soft: rgb(61, 84, 130),
            accent_fg: rgb(17, 18, 26),
            border: rgb(52, 59, 86),
            border_focus: rgb(122, 162, 247),
            header_bg: rgb(32, 34, 48),
            header_fg: rgb(192, 202, 245),
            selection_bg: rgb(52, 72, 120),
            selection_fg: rgb(255, 255, 255),
            success: rgb(158, 206, 106),
            warning: rgb(224, 175, 104),
            error: rgb(247, 118, 142),
            info: rgb(125, 207, 255),
            connected: rgb(158, 206, 106),
            paired: rgb(187, 154, 247),
            trusted: rgb(192, 130, 247),
            blocked: rgb(247, 118, 142),
            gauge_empty: rgb(41, 44, 60),
            chart: vec![
                rgb(122, 162, 247),
                rgb(158, 206, 106),
                rgb(224, 175, 104),
                rgb(247, 118, 142),
            ],
        }
    }

    fn rose_pine() -> Theme {
        Theme {
            name: "Rosé Pine",
            bg: rgb(25, 23, 36),
            surface: rgb(31, 29, 46),
            surface_alt: rgb(38, 35, 58),
            fg: rgb(224, 222, 244),
            muted: rgb(169, 166, 202),
            dim: rgb(110, 106, 134),
            accent: rgb(235, 188, 186),
            accent_soft: rgb(120, 92, 100),
            accent_fg: rgb(25, 23, 36),
            border: rgb(64, 61, 82),
            border_focus: rgb(235, 188, 186),
            header_bg: rgb(31, 29, 46),
            header_fg: rgb(224, 222, 244),
            selection_bg: rgb(82, 68, 96),
            selection_fg: rgb(255, 255, 255),
            success: rgb(156, 207, 216),
            warning: rgb(246, 193, 119),
            error: rgb(235, 111, 146),
            info: rgb(196, 167, 231),
            connected: rgb(156, 207, 216),
            paired: rgb(196, 167, 231),
            trusted: rgb(235, 188, 186),
            blocked: rgb(235, 111, 146),
            gauge_empty: rgb(38, 35, 58),
            chart: vec![
                rgb(235, 188, 186),
                rgb(156, 207, 216),
                rgb(246, 193, 119),
                rgb(235, 111, 146),
            ],
        }
    }

    fn solarized() -> Theme {
        Theme {
            name: "Solarized",
            bg: rgb(0, 43, 54),
            surface: rgb(7, 54, 66),
            surface_alt: rgb(12, 64, 78),
            fg: rgb(131, 148, 150),
            muted: rgb(147, 161, 161),
            dim: rgb(88, 110, 117),
            accent: rgb(38, 139, 210),
            accent_soft: rgb(27, 88, 128),
            accent_fg: rgb(253, 246, 227),
            border: rgb(28, 78, 92),
            border_focus: rgb(38, 139, 210),
            header_bg: rgb(7, 54, 66),
            header_fg: rgb(253, 246, 227),
            selection_bg: rgb(27, 88, 128),
            selection_fg: rgb(253, 246, 227),
            success: rgb(133, 153, 0),
            warning: rgb(181, 137, 0),
            error: rgb(220, 50, 47),
            info: rgb(42, 161, 152),
            connected: rgb(133, 153, 0),
            paired: rgb(38, 139, 210),
            trusted: rgb(108, 113, 196),
            blocked: rgb(220, 50, 47),
            gauge_empty: rgb(12, 64, 78),
            chart: vec![
                rgb(38, 139, 210),
                rgb(133, 153, 0),
                rgb(181, 137, 0),
                rgb(220, 50, 47),
            ],
        }
    }

    fn light() -> Theme {
        Theme {
            name: "Daylight",
            bg: rgb(247, 249, 252),
            surface: rgb(255, 255, 255),
            surface_alt: rgb(236, 241, 247),
            fg: rgb(32, 42, 58),
            muted: rgb(96, 108, 126),
            dim: rgb(160, 170, 186),
            accent: rgb(28, 110, 190),
            accent_soft: rgb(190, 216, 240),
            accent_fg: rgb(255, 255, 255),
            border: rgb(206, 216, 228),
            border_focus: rgb(28, 110, 190),
            header_bg: rgb(230, 239, 250),
            header_fg: rgb(24, 44, 70),
            selection_bg: rgb(206, 228, 248),
            selection_fg: rgb(16, 40, 70),
            success: rgb(22, 130, 84),
            warning: rgb(170, 110, 10),
            error: rgb(190, 50, 55),
            info: rgb(28, 110, 190),
            connected: rgb(22, 130, 84),
            paired: rgb(70, 90, 190),
            trusted: rgb(130, 70, 170),
            blocked: rgb(190, 50, 55),
            gauge_empty: rgb(224, 231, 240),
            chart: vec![
                rgb(28, 110, 190),
                rgb(22, 130, 84),
                rgb(170, 110, 10),
                rgb(190, 50, 55),
            ],
        }
    }
}
