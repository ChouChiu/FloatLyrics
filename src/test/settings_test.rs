use super::*;

#[test]
fn language_indices_follow_the_public_language_order() {
    for (index, language) in Language::ALL.into_iter().enumerate() {
        assert_eq!(language_index(language), index as u32);
    }
}
