use novex_ai_core::{ContextPriority, TrustLevel};
use novex_api::agents::llm::{
    ScriptLLMOutput, ScriptNodeInput, ScriptNodeInputBuilder, ScriptSceneLLMOutput,
};
use novex_api::domain::script::{ScriptGenerationInput, ScriptStyle};
use serde_json::json;
use uuid::Uuid;

fn render(input: &ScriptNodeInput) -> String {
    input
        .context
        .iter()
        .map(|fragment| fragment.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_atomic_context(input: &ScriptNodeInput) {
    assert_eq!(input.context.len(), 3);
    for (render_order, fragment) in input.context.iter().enumerate() {
        assert_eq!(fragment.source_kind, "user_instruction");
        assert_eq!(fragment.trust, TrustLevel::UserInstruction);
        assert_eq!(fragment.priority, ContextPriority::P0);
        assert!(fragment.required);
        assert_eq!(fragment.render_order, render_order as u32);
    }
}

#[test]
fn script_node_input_includes_topic_style_and_scene_count() {
    let request = ScriptGenerationInput {
        project_id: Uuid::new_v4(),
        topic: "ChatGPT如何改变程序员工作流".to_string(),
        topic_id: None,
        style: Some(ScriptStyle::Tutorial),
        scene_count: Some(7),
        parent_id: None,
    };

    let input = ScriptNodeInputBuilder::build(&request);

    assert_atomic_context(&input);
    assert_eq!(
        input
            .context
            .iter()
            .map(|fragment| fragment.key.as_str())
            .collect::<Vec<_>>(),
        ["request", "constraints", "output_example"]
    );
    assert_eq!(
        render(&input),
        r#"请根据以下选题生成7个分镜的中文短视频脚本。

选题：ChatGPT如何改变程序员工作流
风格：教程讲解类（tutorial）

输出要求：
1. 标题不超过30个中文字符。
2. hook 必须能在前3秒抓住观众注意力。
3. 必须严格输出 7 个分镜，sequence 从 1 连续递增。
4. 每个分镜包含 narration、visual_description、emotion、duration_sec。
5. 每个分镜 narration 为 50-150 个中文字符，不能少于50字。
6. 每个分镜 duration_sec 为 1-30 秒，总时长建议 45-60 秒。

JSON Schema：
{
  "title": "标题",
  "hook": "前3秒吸引点",
  "scenes": [
    {
      "sequence": 1,
      "narration": "旁白文本",
      "visual_description": "视觉描述",
      "emotion": "情绪标签",
      "duration_sec": 8
    }
  ]
}"#
    );
}

#[test]
fn script_node_input_marks_parent_requests_as_variants() {
    let request = ScriptGenerationInput {
        project_id: Uuid::new_v4(),
        topic: "ChatGPT如何改变程序员工作流".to_string(),
        topic_id: None,
        style: Some(ScriptStyle::Knowledge),
        scene_count: Some(5),
        parent_id: Some(Uuid::new_v4()),
    };

    let input = ScriptNodeInputBuilder::build(&request);

    assert!(render(&input).contains("差异化版本"));
    assert!(render(&input).contains("避免复用相同表达"));
}

#[test]
fn script_node_input_builder_can_create_metadata_and_scene_inputs() {
    let request = ScriptGenerationInput {
        project_id: Uuid::new_v4(),
        topic: "AI 如何改变人类，人类该如何接受 AI".to_string(),
        topic_id: None,
        style: Some(ScriptStyle::Knowledge),
        scene_count: Some(6),
        parent_id: None,
    };

    let metadata_input = ScriptNodeInputBuilder::build_metadata(&request);
    assert_atomic_context(&metadata_input);
    assert_eq!(
        metadata_input
            .context
            .iter()
            .map(|fragment| fragment.key.as_str())
            .collect::<Vec<_>>(),
        ["request", "constraints", "output_example"]
    );
    assert_eq!(
        render(&metadata_input),
        r#"请根据以下选题生成中文短视频脚本的标题和 hook。只输出 title 和 hook，不要输出 scenes。

选题：AI 如何改变人类，人类该如何接受 AI
风格：知识科普类（knowledge）

输出要求：
1. title 不超过30个中文字符。
2. hook 必须能在前3秒抓住观众注意力。
3. title 和 hook 必须贴合选题，不要泛泛而谈。
4. 必须只输出合法 JSON。

JSON Schema：
{
  "title": "标题",
  "hook": "前3秒吸引点"
}"#
    );

    let scene_input = ScriptNodeInputBuilder::build_single_scene(&request, 4);
    assert_atomic_context(&scene_input);
    assert_eq!(
        scene_input
            .context
            .iter()
            .map(|fragment| fragment.key.as_str())
            .collect::<Vec<_>>(),
        [
            "scene-4-request",
            "scene-4-constraints",
            "scene-4-output-example"
        ]
    );
    assert_eq!(
        render(&scene_input),
        r#"请根据以下选题生成一个中文短视频分镜。只输出单个 scene 对象，不要输出 title、hook 或 scenes 数组。

选题：AI 如何改变人类，人类该如何接受 AI
风格：知识科普类（knowledge）
整体分镜数：6
当前分镜序号：4

输出要求：
1. scene.sequence 必须等于 4。
2. scene 必须包含 narration、visual_description、emotion、duration_sec。
3. narration 为 50-150 个中文字符，不能少于50字。
4. visual_description 必须具体描述画面、人物、动作或字幕。
5. duration_sec 为 1-30 秒。
6. 必须只输出合法 JSON。

JSON Schema：
{
  "scene": {
    "sequence": 4,
    "narration": "旁白文本",
    "visual_description": "视觉描述",
    "emotion": "情绪标签",
    "duration_sec": 8
  }
}"#
    );
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
