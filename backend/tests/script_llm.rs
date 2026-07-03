use novex_api::agents::llm::{ScriptLLMOutput, ScriptPromptBuilder, ScriptSceneLLMOutput};
use novex_api::agents::models::{GenerateScriptRequest, ScriptStyle};
use serde_json::json;
use uuid::Uuid;

#[test]
fn script_prompt_builder_includes_topic_style_and_scene_count() {
    let request = GenerateScriptRequest {
        project_id: Uuid::new_v4(),
        topic: "ChatGPT如何改变程序员工作流".to_string(),
        style: Some(ScriptStyle::Tutorial),
        scene_count: Some(7),
        parent_id: None,
    };

    let prompt = ScriptPromptBuilder::build(&request);

    assert!(prompt.system.contains("短视频脚本创作者"));
    assert!(prompt.user.contains("ChatGPT如何改变程序员工作流"));
    assert!(prompt.user.contains("教程讲解类"));
    assert!(prompt.user.contains("7个分镜"));
    assert!(prompt.user.contains("narration 为 50-150 个中文字符"));
}

#[test]
fn script_prompt_builder_marks_parent_requests_as_variants() {
    let request = GenerateScriptRequest {
        project_id: Uuid::new_v4(),
        topic: "ChatGPT如何改变程序员工作流".to_string(),
        style: Some(ScriptStyle::Knowledge),
        scene_count: Some(5),
        parent_id: Some(Uuid::new_v4()),
    };

    let prompt = ScriptPromptBuilder::build(&request);

    assert!(prompt.user.contains("差异化版本"));
    assert!(prompt.user.contains("避免复用相同表达"));
}

#[test]
fn script_prompt_builder_can_create_small_metadata_and_scene_prompts() {
    let request = GenerateScriptRequest {
        project_id: Uuid::new_v4(),
        topic: "AI 如何改变人类，人类该如何接受 AI".to_string(),
        style: Some(ScriptStyle::Knowledge),
        scene_count: Some(6),
        parent_id: None,
    };

    let metadata_prompt = ScriptPromptBuilder::build_metadata(&request);
    assert!(metadata_prompt.user.contains("只输出 title 和 hook"));
    assert!(!metadata_prompt.user.contains("\"scenes\": ["));
    assert_eq!(metadata_prompt.max_output_tokens, Some(400));

    let scene_prompt = ScriptPromptBuilder::build_single_scene(&request, 4);
    assert!(scene_prompt.user.contains("当前分镜序号：4"));
    assert!(scene_prompt.user.contains("scene.sequence 必须等于 4"));
    assert!(scene_prompt.user.contains("只输出单个 scene 对象"));
    assert_eq!(scene_prompt.max_output_tokens, Some(1_200));
}

#[test]
fn script_llm_output_parses_markdown_wrapped_json_and_validates_scene_count() {
    let raw = r#"
```json
{
  "title": "程序员必看：ChatGPT工作流",
  "hook": "还在手写重复代码？",
  "scenes": [
    {
      "sequence": 1,
      "narration": "传统程序员每天要写大量重复代码，复制粘贴改参数，枯燥又容易出错，团队还要花很多时间检查这些重复劳动带来的隐藏问题。",
      "visual_description": "程序员盯着屏幕，快速切换多个代码文件。",
      "emotion": "焦虑",
      "duration_sec": 8
    },
    {
      "sequence": 2,
      "narration": "现在只要描述需求，AI 就能快速生成初稿，让开发者把时间放回设计和验证，从重复劳动转向架构判断、边界测试和真实业务理解。",
      "visual_description": "屏幕上弹出代码建议，程序员露出惊喜表情。",
      "emotion": "惊喜",
      "duration_sec": 9
    }
  ]
}
```
"#;

    let output = ScriptLLMOutput::parse_and_validate(raw, 2).unwrap();

    assert_eq!(output.title, "程序员必看：ChatGPT工作流");
    assert_eq!(output.scenes.len(), 2);
    assert_eq!(output.scenes[0].sequence, 1);
}

#[test]
fn script_llm_output_rejects_non_contiguous_scenes() {
    let raw = json!({
        "title": "程序员必看：ChatGPT工作流",
        "hook": "还在手写重复代码？",
        "scenes": [
            {
                "sequence": 1,
                "narration": "传统程序员每天要写大量重复代码，复制粘贴改参数，枯燥又容易出错，团队还要花很多时间检查这些重复劳动带来的隐藏问题。",
                "visual_description": "程序员盯着屏幕，快速切换多个代码文件。",
                "emotion": "焦虑",
                "duration_sec": 8
            },
            {
                "sequence": 3,
                "narration": "AI 生成代码初稿后，程序员重点检查边界条件和安全问题，把更多精力放在验证质量、审视取舍以及确保结果符合真实需求。",
                "visual_description": "开发者审阅测试结果和代码 diff。",
                "emotion": "平静",
                "duration_sec": 9
            }
        ]
    })
    .to_string();

    let error = ScriptLLMOutput::parse_and_validate(&raw, 2).unwrap_err();
    assert!(error.to_string().contains("sequence"));
}

#[test]
fn script_llm_output_rejects_text_outside_business_limits() {
    let long_title = "这个标题明显超过三十个中文字符不应该被系统接受否则前端展示会失控";
    let too_short_narration = "旁白太短";
    let raw = json!({
        "title": long_title,
        "hook": "还在手写重复代码？",
        "scenes": [
            {
                "sequence": 1,
                "narration": too_short_narration,
                "visual_description": "程序员盯着屏幕，快速切换多个代码文件。",
                "emotion": "焦虑",
                "duration_sec": 8
            }
        ]
    })
    .to_string();

    let error = ScriptLLMOutput::parse_and_validate(&raw, 1).unwrap_err();

    assert!(error.to_string().contains("title"));

    let raw = json!({
        "title": "程序员必看：ChatGPT工作流",
        "hook": "还在手写重复代码？",
        "scenes": [
            {
                "sequence": 1,
                "narration": too_short_narration,
                "visual_description": "程序员盯着屏幕，快速切换多个代码文件。",
                "emotion": "焦虑",
                "duration_sec": 8
            }
        ]
    })
    .to_string();

    let error = ScriptLLMOutput::parse_and_validate(&raw, 1).unwrap_err();

    assert!(error.to_string().contains("narration"));
}

#[test]
fn script_scene_llm_output_parses_and_validates_expected_sequence() {
    let raw = json!({
        "scene": {
            "sequence": 3,
            "narration": "AI 正在把重复劳动交给机器，把判断、创意和同理心留给人类。接受 AI 的关键，是学会提问、验证结果，并把它当成放大能力的工具。",
            "visual_description": "人类和 AI 在同一张工作台前协作，屏幕显示分析结果和人工确认标记。",
            "emotion": "理性",
            "duration_sec": 9
        }
    })
    .to_string();

    let output = ScriptSceneLLMOutput::parse_and_validate(&raw, 3).unwrap();

    assert_eq!(output.scene.sequence, 3);
    assert!(output.scene.narration.contains("接受 AI"));
}

#[test]
fn script_scene_llm_output_rejects_wrong_sequence() {
    let raw = json!({
        "scene": {
            "sequence": 2,
            "narration": "AI 正在把重复劳动交给机器，把判断、创意和同理心留给人类。接受 AI 的关键，是学会提问、验证结果，并把它当成放大能力的工具。",
            "visual_description": "人类和 AI 在同一张工作台前协作，屏幕显示分析结果和人工确认标记。",
            "emotion": "理性",
            "duration_sec": 9
        }
    })
    .to_string();

    let error = ScriptSceneLLMOutput::parse_and_validate(&raw, 3).unwrap_err();

    assert!(error.to_string().contains("expected sequence 3"));
}
