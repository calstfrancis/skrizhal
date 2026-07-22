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

/// Fields every entry has as a first-class struct field, rather than in the
/// open `extra` map. Recommended fields outside this set are what a category
/// actually adds — the ones a form has to conjure a row for.
pub const COMMON_FIELDS: &[&str] = &["organization", "location", "date", "description", "tags"];

/// The recommended fields for `category` that live in `extra` rather than
/// being common to every entry (e.g. `degree` for Education, `venue` for
/// Publication). Empty for an unknown category — there's nothing to suggest.
pub fn category_specific_fields(category: &str) -> &'static [&'static str] {
    match lookup(category) {
        Some(spec) => spec.recommended_fields,
        None => &[],
    }
}

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
    fn category_specific_fields_excludes_common_ones() {
        let fields = category_specific_fields("Education");
        assert!(fields.contains(&"degree"));
        // The caller filters COMMON_FIELDS out; the registry itself still
        // lists them, since validation checks those too.
        assert!(fields.contains(&"organization"));
        assert!(COMMON_FIELDS.contains(&"organization"));
        assert!(!COMMON_FIELDS.contains(&"degree"));
    }

    #[test]
    fn category_specific_fields_for_unknown_category_is_empty() {
        assert!(category_specific_fields("Not A Real Category").is_empty());
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
