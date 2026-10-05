//! Program maps of real example programs.

use std::path::Path;

use pina_cli::map::ProjectMap;
use pina_cli::map::map_project;
use pina_cli::map::render_html;

fn example(name: &str) -> ProjectMap {
	let path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../examples")
		.join(name);

	map_project(&path).unwrap_or_else(|error| panic!("map for {name} failed: {error}"))
}

#[test]
fn counter_map_embeds_the_documented_data() {
	let project = example("counter_program");

	assert!(project.default_output.ends_with("pina/map.html"));
	insta::assert_json_snapshot!("counter_program_map", project.map);
}

#[test]
fn privacy_pool_map_renders_every_instruction_and_hotspot() {
	let project = example("privacy_pool_program");
	let html = render_html(&project.map).unwrap_or_else(|error| panic!("render failed: {error}"));

	assert_eq!(project.map.instruction_details.len(), 13);
	assert_eq!(project.map.locks.hotspots.len(), 7);
	assert!(html.contains("<title>privacy_pool_program · interlocking chart</title>"));
	assert!(html.contains("\"name\":\"merkle_tree\""));
	assert!(html.contains("BLquaQVntisnQpUoLpzbcFGDN9eG9Qf12TZZS5KVLaLz"));
}
