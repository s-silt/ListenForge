use serde_json::{json, Value};

/// 生成 ExtractedScript 的 JSON Schema。
/// 每个 object additionalProperties:false,所有属性入 required,
/// Option<T> 用 ["T","null"] 类型,task_type 用字符串 enum(七值)。
pub fn extracted_script_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "parts"],
        "properties": {
            "title": {
                "type": ["string", "null"]
            },
            "parts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["label", "task_type", "zh_instruction", "items"],
                    "properties": {
                        "label": {
                            "type": "string"
                        },
                        "task_type": {
                            "type": "string",
                            "enum": [
                                "listen_and_choose",
                                "listen_and_number",
                                "listen_and_judge",
                                "listen_and_write",
                                "listen_and_circle",
                                "listen_passage",
                                "unknown"
                            ]
                        },
                        "zh_instruction": {
                            "type": ["string", "null"]
                        },
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["number", "text", "speaker"],
                                "properties": {
                                    "number": {
                                        "type": ["integer", "null"]
                                    },
                                    "text": {
                                        "type": "string"
                                    },
                                    "speaker": {
                                        "type": ["string", "null"]
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_top_level_required_and_no_additional_props() {
        let schema = extracted_script_schema();
        let required = schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_strs.contains(&"title"));
        assert!(required_strs.contains(&"parts"));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn schema_task_type_enum_contains_listen_passage() {
        let schema = extracted_script_schema();
        let enum_vals = schema["properties"]["parts"]["items"]["properties"]["task_type"]["enum"]
            .as_array()
            .unwrap();
        let vals: Vec<&str> = enum_vals.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(vals.contains(&"listen_passage"), "应包含 listen_passage, 实际: {:?}", vals);
        assert_eq!(vals.len(), 7, "task_type enum 应有 7 个值");
    }

    #[test]
    fn schema_parts_item_additional_properties_false() {
        let schema = extracted_script_schema();
        assert_eq!(
            schema["properties"]["parts"]["items"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn schema_items_item_additional_properties_false() {
        let schema = extracted_script_schema();
        assert_eq!(
            schema["properties"]["parts"]["items"]["properties"]["items"]["items"]
                ["additionalProperties"],
            false
        );
    }

    #[test]
    fn schema_extracted_item_has_speaker_field() {
        let schema = extracted_script_schema();
        let item_schema = &schema["properties"]["parts"]["items"]["properties"]["items"]["items"];
        // speaker should be in required
        let required = item_schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required_strs.contains(&"speaker"), "speaker 应在 required 中, 实际: {:?}", required_strs);
        // speaker should be nullable string
        let speaker_type = &item_schema["properties"]["speaker"]["type"];
        let types: Vec<&str> = speaker_type.as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert!(types.contains(&"string"), "speaker type 应含 string");
        assert!(types.contains(&"null"), "speaker type 应含 null");
    }
}
