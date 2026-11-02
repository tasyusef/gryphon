//! The sidebar's marks: a 12×12 bitmap for each list (X = ink), drawn 2px a cell into the 24px
//! slot, the way Toolbox draws its tools' marks (`ToolIcon` in its ui-kit).

use leptos::prelude::*;

use crate::app::Screen;

/// A star: the addons picked by hand.
const PICKS: [&str; 12] = [
    ".....XX.....",
    ".....XX.....",
    "....XXXX....",
    "....XXXX....",
    "XXXXXXXXXXXX",
    ".XXXXXXXXXX.",
    "..XXXXXXXX..",
    "...XXXXXX...",
    "...XXXXXX...",
    "..XXX..XXX..",
    "..XX....XX..",
    ".XX......XX.",
];

/// A branch: any addon from a GitHub repo.
const GITHUB: [&str; 12] = [
    "..XX....XX..",
    ".X..X..X..X.",
    ".X..X..X..X.",
    "..XX....XX..",
    "..XX....XX..",
    "..XX...XX...",
    "..XX.XXX....",
    "..XXXX......",
    "..XX........",
    ".X..X.......",
    ".X..X.......",
    "..XX........",
];

/// A folder: the other folders in AddOns.
const FOLDERS: [&str; 12] = [
    "............",
    "XXXXX.......",
    "X...XX......",
    "X....XXXXXXX",
    "X..........X",
    "X..........X",
    "X..........X",
    "X..........X",
    "X..........X",
    "X..........X",
    "XXXXXXXXXXXX",
    "............",
];

/// The mark for a list in the sidebar; Settings has none, like Toolbox's footer rows.
pub fn rows(screen: Screen) -> Option<&'static [&'static str; 12]> {
    match screen {
        Screen::Picks => Some(&PICKS),
        Screen::Github => Some(&GITHUB),
        Screen::Folders => Some(&FOLDERS),
        Screen::Settings => None,
    }
}

/// One square per X, as an SVG path in cells.
fn path(rows: &[&str]) -> String {
    let mut d = String::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, cell) in row.chars().enumerate() {
            if cell == 'X' {
                d.push_str(&format!("M{x} {y}h1v1h-1z"));
            }
        }
    }
    d
}

#[component]
pub fn Mark(rows: &'static [&'static str; 12]) -> impl IntoView {
    view! {
        <span class="side-mark">
            <svg aria-hidden="true" shape-rendering="crispEdges" viewBox="0 0 12 12" width="24" height="24">
                <path fill="currentColor" d=path(rows)></path>
            </svg>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mark_is_twelve_cells_square() {
        for screen in [Screen::Picks, Screen::Github, Screen::Folders] {
            let rows = rows(screen).unwrap();
            assert!(rows.iter().all(|row| row.len() == 12 && row.chars().all(|c| c == '.' || c == 'X')), "{screen:?}");
        }
        assert!(rows(Screen::Settings).is_none());
    }

    #[test]
    fn one_square_per_x() {
        assert_eq!(path(&["X.", ".X"]), "M0 0h1v1h-1zM1 1h1v1h-1z");
    }
}
