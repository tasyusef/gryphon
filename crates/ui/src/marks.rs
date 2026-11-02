//! The sidebar's marks: each list opens with an initial, the way a scribe opened a section, set
//! in the blackletter face on the list's own colour (brand.css).

use leptos::prelude::*;

use crate::app::Screen;

/// The letter for a list in the sidebar and the class that colours it; Settings has none, like
/// Toolbox's footer rows.
pub fn initial(screen: Screen) -> Option<(&'static str, &'static str)> {
    match screen {
        Screen::Picks => Some(("P", "tint-picks")),
        Screen::Github => Some(("G", "tint-github")),
        Screen::Folders => Some(("F", "tint-folders")),
        Screen::Settings => None,
    }
}

#[component]
pub fn Mark(letter: &'static str, tint: &'static str) -> impl IntoView {
    view! { <span class=format!("side-mark init {tint}") aria-hidden="true">{letter}</span> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_list_opens_with_its_own_first_letter() {
        for screen in [Screen::Picks, Screen::Github, Screen::Folders] {
            let (letter, _) = initial(screen).unwrap();
            assert!(screen.name().starts_with(letter), "{screen:?}");
        }
        assert!(initial(Screen::Settings).is_none());
    }
}
