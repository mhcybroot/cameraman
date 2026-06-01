use serde_json::json;

/// Generates a customized AI analysis prompt based on a list of enabled modules.
/// If the list is empty, it defaults to the ANPR (license plate) module.
pub fn build_cctv_prompt(enabled_modules: &[String]) -> String {
    let mut modules = enabled_modules.to_vec();
    if modules.is_empty() {
        modules.push("anpr".to_string());
    }

    let mut instructions = String::new();
    let mut schema_fields = serde_json::Map::new();

    for module in &modules {
        match module.to_lowercase().as_str() {
            "anpr" => {
                instructions.push_str("- **anpr**: Extract the Bangladeshi vehicle license plate from this image. Bangladeshi license plates have two lines: Top Line (e.g. 'ঢাকা মেট্রো ঘ' or 'সিলেট হ') containing the district, optionally the word 'মেট্রো', and a single vehicle class letter (ক to হ); Bottom Line (e.g. '১২-৩৪৫৬') containing exactly 6 digits formatted as XX-XXXX.\n");
                schema_fields.insert(
                    "anpr".to_string(),
                    json!({
                        "plate_found": "boolean",
                        "top_line": "string or null",
                        "bottom_line": "string or null",
                        "raw_text": "string or null",
                        "vehicle_context": "string (e.g., red sedan, blue motorcycle)"
                    }),
                );
            }
            "face_detection" => {
                instructions.push_str("- **face_detection**: Detect all human faces in the image. For each face, specify the approximate bounding box [ymin, xmin, ymax, xmax] in percentages (0-100), and estimate attributes: age (e.g., '20-30', '40-50'), gender ('male', 'female', 'unknown'), and emotion ('happy', 'sad', 'angry', 'neutral').\n");
                schema_fields.insert(
                    "face_detection".to_string(),
                    json!({
                        "detected": "boolean",
                        "count": "integer",
                        "details": [
                            {
                                "bbox": ["ymin_pct", "xmin_pct", "ymax_pct", "xmax_pct"],
                                "attributes": { "age": "string", "gender": "string", "emotion": "string" }
                            }
                        ]
                    }),
                );
            }
            "perimeter_protection" => {
                instructions.push_str("- **perimeter_protection**: Identify if any humans, vehicles, or animals have crossed into restricted perimeter zones in this image. Assume a perimeter breach if objects are in forbidden spaces.\n");
                schema_fields.insert(
                    "perimeter_protection".to_string(),
                    json!({
                        "breach": "boolean",
                        "zone_name": "string (e.g., restricted_gate, fence_line, or null)",
                        "objects": ["string (e.g., person, vehicle)"]
                    }),
                );
            }
            "face_recognition" => {
                instructions.push_str("- **face_recognition**: Identify and match any detected faces against known personnel in the facility (VIPs, staff, blacklisted individuals). If a match is found, list the name and role; otherwise, label as 'unknown'.\n");
                schema_fields.insert(
                    "face_recognition".to_string(),
                    json!({
                        "matches": [
                            { "name": "string", "confidence": "float (0-1)", "role": "string (staff, visitor, blacklist, unknown)" }
                        ]
                    }),
                );
            }
            "video_metadata" => {
                instructions.push_str("- **video_metadata**: Extract structured attributes for humans, motor vehicles, and non-motor vehicles present in the image. For humans: clothing color, backpack presence. For motor vehicles: vehicle type (car, truck, bus, bike), color, brand/model. For non-motor vehicles: type (bicycle, rickshaw).\n");
                schema_fields.insert(
                    "video_metadata".to_string(),
                    json!({
                        "humans": [
                            { "gender": "string", "upper_clothing_color": "string", "lower_clothing_color": "string", "backpack": "boolean" }
                        ],
                        "motor_vehicles": [
                            { "type": "string (car, truck, motorcycle, bus)", "color": "string", "brand": "string or null" }
                        ],
                        "non_motor_vehicles": [
                            { "type": "string (bicycle, rickshaw, cart)" }
                        ]
                    }),
                );
            }
            "smd_plus" => {
                instructions.push_str("- **smd_plus**: Perform Smart Motion Detection (SMD Plus) to filter motion events. Identify if the primary motion trigger is a 'human', 'vehicle', or 'noise' (wind, rain, tree leaves, animal).\n");
                schema_fields.insert(
                    "smd_plus".to_string(),
                    json!({
                        "trigger_type": "string (human, vehicle, noise)",
                        "confidence": "float (0-1)",
                        "description": "string"
                    }),
                );
            }
            "stereo_analysis" => {
                instructions.push_str("- **stereo_analysis**: Analyze depth and height in the scene to detect anomalous behavior (e.g. fall detection, climbing over barriers, height violations).\n");
                schema_fields.insert(
                    "stereo_analysis".to_string(),
                    json!({
                        "anomaly_detected": "boolean",
                        "anomaly_type": "string (fall, climbing, none)",
                        "estimated_height_m": "float or null",
                        "details": "string or null"
                    }),
                );
            }
            "crowd_distribution" => {
                instructions.push_str("- **crowd_distribution**: Analyze crowd density and distribution in the area.\n");
                schema_fields.insert(
                    "crowd_distribution".to_string(),
                    json!({
                        "crowd_level": "string (low, medium, high, critical)",
                        "estimated_count": "integer",
                        "density_percentage": "integer (0-100)"
                    }),
                );
            }
            "people_counting" => {
                instructions.push_str("- **people_counting**: Count the total number of people in the image, estimating those moving 'in' (entering/crossing forward) vs 'out' (exiting/crossing backward).\n");
                schema_fields.insert(
                    "people_counting".to_string(),
                    json!({
                        "current_count": "integer",
                        "entered": "integer",
                        "exited": "integer"
                    }),
                );
            }
            "vehicle_density" => {
                instructions.push_str("- **vehicle_density**: Evaluate the traffic congestion and vehicle count in the camera view.\n");
                schema_fields.insert(
                    "vehicle_density".to_string(),
                    json!({
                        "congestion_level": "string (clear, moderate, heavy, standstill)",
                        "vehicle_count": "integer",
                        "density_percentage": "integer (0-100)"
                    }),
                );
            }
            "heat_map" => {
                instructions.push_str("- **heat_map**: Generate regional coordinate hotspots representing motion density. Specify high-activity coordinates in percentage grid cells (e.g., [x, y] cells from 0 to 9).\n");
                schema_fields.insert(
                    "heat_map".to_string(),
                    json!({
                        "hotspots": [ ["integer (x_cell)", "integer (y_cell)"] ],
                        "description": "string"
                    }),
                );
            }
            "ppe_detection" => {
                instructions.push_str("- **ppe_detection**: Scan all people in the image for Personal Protective Equipment (PPE) compliance. Check for: 'hardhat' (safety helmet), 'safety_vest' (high-visibility vest), 'mask' (face mask), and 'boots'.\n");
                schema_fields.insert(
                    "ppe_detection".to_string(),
                    json!({
                        "violations_found": "boolean",
                        "details": [
                            {
                                "person_index": "integer",
                                "has_hardhat": "boolean",
                                "has_safety_vest": "boolean",
                                "has_mask": "boolean",
                                "missing_items": ["string"]
                            }
                        ]
                    }),
                );
            }
            "smart_object_detection" => {
                instructions.push_str("- **smart_object_detection**: Detect smart objects alerts: 'loitering' (person staying in a place too long), 'abandoned_object' (e.g., luggage left behind), or 'missing_object' (removed asset).\n");
                schema_fields.insert(
                    "smart_object_detection".to_string(),
                    json!({
                        "alert_triggered": "boolean",
                        "alert_type": "string (loitering, abandoned_object, missing_object, none)",
                        "details": "string or null"
                    }),
                );
            }
            "smart_sound_detection" => {
                instructions.push_str("- **smart_sound_detection**: Based on visual indicators in the image (e.g., a person screaming, glass broken on the floor, crowd running in panic), evaluate potential sound alerts.\n");
                schema_fields.insert(
                    "smart_sound_detection".to_string(),
                    json!({
                        "sound_alert_triggered": "boolean",
                        "inferred_sound_type": "string (scream, glass_break, explosion, none)",
                        "confidence": "float (0-1)",
                        "details": "string or null"
                    }),
                );
            }
            _ => {}
        }
    }

    let schema_json = serde_json::Value::Object(schema_fields);

    format!(
        "You are an advanced CCTV AI vision processor. Analyze the provided image according to these active modules:\n\n\
         {}\n\
         You MUST respond ONLY with a valid JSON object matching the following structure. Do not output any markdown code fences (like ```json), no comments, no extra text, and no whitespace outside standard JSON serialization. \
         The JSON must match this schema:\n\n\
         {}",
        instructions,
        serde_json::to_string_pretty(&schema_json).unwrap_or_default()
    )
}
