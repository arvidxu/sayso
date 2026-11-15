use dictation_types::{FeatureDescriptor, FeatureId, FeatureMountPoint};

#[test]
fn a_new_feature_entry_belongs_to_nobody_and_has_no_route() {
    let feature = FeatureDescriptor::new(
        FeatureId::new("history"),
        "nav.history",
        FeatureMountPoint::NavigationRoute,
        true,
    );
    assert_eq!(feature.extension_id, None);
    assert_eq!(feature.route_path, None);
    assert_eq!(feature.order, 0);
    assert_eq!(feature.mount_point, FeatureMountPoint::NavigationRoute);
    assert_eq!(feature.title_key, "nav.history");
}

#[test]
fn a_feature_entry_keeps_the_route_and_order_it_was_given() {
    let feature = FeatureDescriptor::new(
        FeatureId::new("glossary"),
        "settings.glossary",
        FeatureMountPoint::SettingsSection,
        true,
    )
    .with_route("/settings/glossary")
    .with_order(-5);
    assert_eq!(feature.route_path.as_deref(), Some("/settings/glossary"));
    assert_eq!(feature.order, -5);
}
