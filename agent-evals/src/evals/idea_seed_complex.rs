#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::project_intake::seed_project_from_idea;
use std::path::Path;

pub fn run_idea_seed_complex_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();

    // Complex setting: dual power system + nested world + faction relationships
    let text = concat!(
        "修炼体系：内丹法分三脉七轮，每轮对应一种元素之力；外丹法依赖天材地宝炼制丹药，可改变体质。\n",
        "世界观：表层世界为凡人江湖，各大门派争夺资源。里层世界为修真界，每百年通过'天门'开启一次通道。\n",
        "力量等级：凡人→炼气→筑基→金丹→元婴→化神，每级分初期、中期、后期、圆满四段。\n",
        "角色：\n",
        "林墨——内丹法修炼者，金丹中期，背负寒玉戒指令牌的秘密。\n",
        "苏云——外丹法大师，表面上经营药铺，实则暗中收集天材地宝。\n",
        "影子宗——潜伏在表里世界的隐秘组织，操控天门开启时间。\n",
        "冲突：人族与妖族表面和平，实则互相渗透。内丹派与外丹派因资源争夺而对立。\n",
        "主线：林墨追寻寒玉戒指的真相，却发现戒指是天门的钥匙之一，而影子宗正在策划提前开启天门。",
    );

    let report = seed_project_from_idea(&memory, "eval-complex", text).unwrap();

    let has_chars = !report.identified_characters.is_empty();
    let has_canon = !report.identified_canon.is_empty();
    let has_promises = !report.open_promises.is_empty();
    let has_conflicts = !report.conflicts.is_empty();
    let recs = report.recommendations.len();

    let comprehensive = has_chars && has_canon && has_promises && has_conflicts;
    EvalResult::pass_if(
        "idea_seed_complex",
        comprehensive,
        format!(
            "chars={} canon={} promises={} conflicts={} recs={} conf={:.2}",
            report.identified_characters.len(),
            report.identified_canon.len(),
            report.open_promises.len(),
            report.conflicts.len(),
            recs,
            report.confidence
        ),
    )
}
