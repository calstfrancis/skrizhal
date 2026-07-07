/// Describes one CV entry type: its display name and which fields are
/// expected for it. Drives soft-validation warnings now and per-type GUI
/// form layout later — new types only need an entry here, no struct change,
/// since `CvEntry` stores type-specific fields in an open `extra` map.
pub struct TypeSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Field names considered recommended for this type. May reference
    /// either a common `CvEntry` field (`organization`, `location`, `date`,
    /// `description`, `tags`) or a type-specific key expected in `extra`.
    pub recommended_fields: &'static [&'static str],
}

/// Namespaced type registry. Values are kebab-case and used verbatim as the
/// entry's `type:` field in YAML.
pub const TYPE_REGISTRY: &[TypeSpec] = &[
    TypeSpec {
        id: "education",
        display_name: "Education",
        recommended_fields: &["organization", "date", "degree"],
    },
    TypeSpec {
        id: "employment",
        display_name: "Employment",
        recommended_fields: &["organization", "location", "date", "description"],
    },
    TypeSpec {
        id: "ministry-position",
        display_name: "Ministry Position",
        recommended_fields: &["organization", "location", "date", "description"],
    },
    TypeSpec {
        id: "publication",
        display_name: "Publication",
        recommended_fields: &["date", "venue"],
    },
    TypeSpec {
        id: "presentation",
        display_name: "Presentation",
        recommended_fields: &["date", "event-name"],
    },
    TypeSpec {
        id: "award",
        display_name: "Award",
        recommended_fields: &["organization", "date"],
    },
    TypeSpec {
        id: "service",
        display_name: "Service",
        recommended_fields: &["organization", "date", "role"],
    },
    TypeSpec {
        id: "committee-appointment",
        display_name: "Committee Appointment",
        recommended_fields: &["organization", "date", "role"],
    },
    TypeSpec {
        id: "language-skill",
        display_name: "Language Skill",
        recommended_fields: &["language", "proficiency"],
    },
    TypeSpec {
        id: "certification",
        display_name: "Certification",
        recommended_fields: &["organization", "date"],
    },
    TypeSpec {
        id: "volunteer",
        display_name: "Volunteer",
        recommended_fields: &["organization", "location", "date", "description"],
    },
    TypeSpec {
        id: "project",
        display_name: "Project",
        recommended_fields: &["date", "description"],
    },
];

pub fn lookup(entry_type: &str) -> Option<&'static TypeSpec> {
    TYPE_REGISTRY.iter().find(|t| t.id == entry_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_type() {
        let spec = lookup("ministry-position").unwrap();
        assert_eq!(spec.display_name, "Ministry Position");
    }

    #[test]
    fn lookup_unknown_type_returns_none() {
        assert!(lookup("not-a-real-type").is_none());
    }

    #[test]
    fn all_ids_are_unique() {
        let mut ids: Vec<&str> = TYPE_REGISTRY.iter().map(|t| t.id).collect();
        let count_before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count_before);
    }
}
