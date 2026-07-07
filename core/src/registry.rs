/// Describes one CV entry category: its canonical name and which fields are
/// expected for it. Drives soft-validation warnings now and per-category GUI
/// form layout later — new categories only need an entry here, no struct
/// change, since `CvEntry` stores category-specific fields in an open
/// `extra` map.
pub struct CategorySpec {
    /// Canonical, Title Case name — this is the actual value stored in
    /// `category:` in YAML, not just a display label. Chosen so the raw
    /// YAML stays human-readable (`category: Ministry Position` rather than
    /// a separate machine id like `ministry-position`).
    pub name: &'static str,
    /// Field names considered recommended for this category. May reference
    /// either a common `CvEntry` field (`organization`, `location`, `date`,
    /// `description`, `tags`) or a category-specific key expected in `extra`.
    pub recommended_fields: &'static [&'static str],
}

pub const CATEGORY_REGISTRY: &[CategorySpec] = &[
    CategorySpec {
        name: "Education",
        recommended_fields: &["organization", "date", "degree"],
    },
    CategorySpec {
        name: "Employment",
        recommended_fields: &["organization", "location", "date", "description"],
    },
    CategorySpec {
        name: "Ministry Position",
        recommended_fields: &["organization", "location", "date", "description"],
    },
    CategorySpec {
        name: "Publication",
        recommended_fields: &["date", "venue"],
    },
    CategorySpec {
        name: "Presentation",
        recommended_fields: &["date", "event-name"],
    },
    CategorySpec {
        name: "Award",
        recommended_fields: &["organization", "date"],
    },
    CategorySpec {
        name: "Service",
        recommended_fields: &["organization", "date", "role"],
    },
    CategorySpec {
        name: "Committee Appointment",
        recommended_fields: &["organization", "date", "role"],
    },
    CategorySpec {
        name: "Language Skill",
        recommended_fields: &["language", "proficiency"],
    },
    CategorySpec {
        name: "Certification",
        recommended_fields: &["organization", "date"],
    },
    CategorySpec {
        name: "Volunteer",
        recommended_fields: &["organization", "location", "date", "description"],
    },
    CategorySpec {
        name: "Project",
        recommended_fields: &["date", "description"],
    },
];

/// Case-insensitive lookup — the registry's own names are the canonical
/// (Title Case) form, but a hand-typed category shouldn't fail to match
/// just because of casing.
pub fn lookup(category: &str) -> Option<&'static CategorySpec> {
    CATEGORY_REGISTRY
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(category))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_category() {
        let spec = lookup("Ministry Position").unwrap();
        assert_eq!(spec.name, "Ministry Position");
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let spec = lookup("ministry position").unwrap();
        assert_eq!(spec.name, "Ministry Position");
    }

    #[test]
    fn lookup_unknown_category_returns_none() {
        assert!(lookup("Not A Real Category").is_none());
    }

    #[test]
    fn all_names_are_unique() {
        let mut names: Vec<&str> = CATEGORY_REGISTRY.iter().map(|c| c.name).collect();
        let count_before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count_before);
    }
}
