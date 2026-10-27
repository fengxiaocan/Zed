# Git Graph Branch Visualization & Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 优化 Zed 的 Git Graph 图形化展示与分支管理体验，实现类似 Android Studio 的最左主干泳道布局、全链路祖先后代追踪高亮与置灰、双分支异同对比（Ahead/Behind/Base 徽章与过滤），以及分支间总差异联动。

**Architecture:** 基于前端内存 DAG 双向索引（`parents` 与 `children_by_parent`）实现微秒级全链路高亮与 Ahead/Behind 集合计算；重构 `GraphData::add_commits` 优先保证 HEAD 分支连续占据 Lane 0；在 `GitGraph` 引入 `BranchCompareState` 与专用对比工具条，无缝串联 Git Manager 分支列表右键操作与两分支汇总 Diff 视图。

**Tech Stack:** Rust, GPUI, Git CLI (`git::Repository`, `project::git_store`), `git_ui`.

---

### Task 1: DAG 双向索引与全链路祖先后代遍历算法 (DAG Bidirectional Index & Lineage Traversal)

**Files:**
- Modify: `crates/git_ui/src/git_graph.rs:889-1065`
- Test: `crates/git_ui/src/git_graph.rs:4750-4820`

- [ ] **Step 1: 编写全链路可达性计算的单元测试 (Failing Test)**

在 [`crates/git_ui/src/git_graph.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/git_graph.rs) 单元测试模块中添加 `test_lineage_traversal`：

```rust
#[gpui::test]
fn test_lineage_traversal() {
    let mut graph_data = GraphData::new(8);
    // 构造分叉拓扑：
    // c4 (head) -> c2 -> c1
    // c3 -> c1
    // c5 (merge of c3 and c2) -> c3, c2
    let c1 = Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    let c2 = Oid::from_str("2222222222222222222222222222222222222222").unwrap();
    let c3 = Oid::from_str("3333333333333333333333333333333333333333").unwrap();
    let c4 = Oid::from_str("4444444444444444444444444444444444444444").unwrap();
    let c5 = Oid::from_str("5555555555555555555555555555555555555555").unwrap();

    let commits = vec![
        Arc::new(InitialGraphCommitData {
            sha: c5,
            parents: smallvec![c3, c2],
            ref_names: vec![],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c4,
            parents: smallvec![c2],
            ref_names: vec!["HEAD".into()],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c3,
            parents: smallvec![c1],
            ref_names: vec![],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c2,
            parents: smallvec![c1],
            ref_names: vec![],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c1,
            parents: smallvec![],
            ref_names: vec![],
        }),
    ];

    graph_data.add_commits(&commits);

    // 查询 c2 的全链路：
    // 祖先应包含 c1
    // 后代应包含 c4 和 c5
    // 因此 c2 的链路应包含 {c1, c2, c4, c5}，但不包含独立的兄弟分支提交 c3
    let lineage = graph_data.find_lineage(c2);
    assert!(lineage.contains(&c1));
    assert!(lineage.contains(&c2));
    assert!(lineage.contains(&c4));
    assert!(lineage.contains(&c5));
    assert!(!lineage.contains(&c3));
}
```

- [ ] **Step 2: 运行测试以确认失败**

运行：`cargo test -p git_ui git_graph::test_lineage_traversal`  
预期：编译失败，提示 `find_lineage` 方法不存在。

- [ ] **Step 3: 实现 `children_by_parent` 反向索引与 `find_lineage` 遍历方法**

在 [`GraphData`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/git_graph.rs#L889-L901) 中新增字段与逻辑：

```rust
struct GraphData {
    // 现有字段...
    children_by_parent: HashMap<Oid, SmallVec<[Oid; 2]>>,
    oid_to_index: HashMap<Oid, usize>,
}

impl GraphData {
    // 在 clear() 中清理 children_by_parent 和 oid_to_index
    // 在 add_commits() 中：
    // 记录 oid_to_index.insert(commit.sha, commit_row);
    // 对 commit.parents 中的每个 parent：
    // children_by_parent.entry(*parent).or_default().push(commit.sha);

    pub fn find_lineage(&self, target_sha: Oid) -> HashSet<Oid> {
        let mut lineage = HashSet::default();
        let mut queue = VecDeque::new();
        queue.push_back(target_sha);
        lineage.insert(target_sha);

        // 向上遍历祖先 (Ancestors)
        while let Some(current_sha) = queue.pop_front() {
            if let Some(&row_idx) = self.oid_to_index.get(&current_sha) {
                if let Some(entry) = self.commits.get(row_idx) {
                    for parent in &entry.data.parents {
                        if lineage.insert(*parent) {
                            queue.push_back(*parent);
                        }
                    }
                }
            }
        }

        // 向下遍历后代 (Descendants)
        queue.push_back(target_sha);
        while let Some(current_sha) = queue.pop_front() {
            if let Some(children) = self.children_by_parent.get(&current_sha) {
                for child in children {
                    if lineage.insert(*child) {
                        queue.push_back(*child);
                    }
                }
            }
        }

        lineage
    }
}
```

- [ ] **Step 4: 运行测试以确认通过**

运行：`cargo test -p git_ui git_graph::test_lineage_traversal`  
预期：PASS。

- [ ] **Step 5: 提交更改**

```bash
git add crates/git_ui/src/git_graph.rs
git commit -m "git_graph: add bidirectional DAG index and lineage traversal"
```

---

### Task 2: 主干（HEAD）优先最左泳道（Lane 0 Priority）拓扑重构

**Files:**
- Modify: `crates/git_ui/src/git_graph.rs:952-1065`
- Test: `crates/git_ui/src/git_graph.rs:4820-4900`

- [ ] **Step 1: 编写主干稳定在 Lane 0 的单元测试**

在 [`crates/git_ui/src/git_graph.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/git_graph.rs) 中添加 `test_lane_0_head_priority`：

```rust
#[gpui::test]
fn test_lane_0_head_priority() {
    let mut graph_data = GraphData::new(8);
    // 构造主干链：c1 <- c2 <- c4 (HEAD)
    // 构造侧分支：c1 <- c3
    let c1 = Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    let c2 = Oid::from_str("2222222222222222222222222222222222222222").unwrap();
    let c3 = Oid::from_str("3333333333333333333333333333333333333333").unwrap();
    let c4 = Oid::from_str("4444444444444444444444444444444444444444").unwrap();

    let commits = vec![
        Arc::new(InitialGraphCommitData {
            sha: c4,
            parents: smallvec![c2],
            ref_names: vec!["HEAD".into(), "refs/heads/main".into()],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c3,
            parents: smallvec![c1],
            ref_names: vec!["refs/heads/feature".into()],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c2,
            parents: smallvec![c1],
            ref_names: vec![],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c1,
            parents: smallvec![],
            ref_names: vec![],
        }),
    ];

    graph_data.add_commits(&commits);

    // 断言主干分支的 c4、c2、c1 均保持分配在 lane 0
    let c4_entry = graph_data.commits.iter().find(|c| c.data.sha == c4).unwrap();
    let c2_entry = graph_data.commits.iter().find(|c| c.data.sha == c2).unwrap();
    let c1_entry = graph_data.commits.iter().find(|c| c.data.sha == c1).unwrap();
    assert_eq!(c4_entry.lane, 0, "HEAD commit should be on lane 0");
    assert_eq!(c2_entry.lane, 0, "Trunk commit c2 should be on lane 0");
    assert_eq!(c1_entry.lane, 0, "Root trunk commit c1 should be on lane 0");

    // 侧分支 c3 应向右分配到 lane 1
    let c3_entry = graph_data.commits.iter().find(|c| c.data.sha == c3).unwrap();
    assert_eq!(c3_entry.lane, 1, "Side branch c3 should be on lane 1");
}
```

- [ ] **Step 2: 运行测试以确认失败**

运行：`cargo test -p git_ui git_graph::test_lane_0_head_priority`  
预期：FAIL（在未重构前，c3 先到达可能会抢占空闲的 Lane 0，导致 c2 漂移到 Lane 1）。

- [ ] **Step 3: 重构泳道分配算法以支持主干 Lane 0 锁定与右向分配**

在 `GraphData` 中增加主干链的预分析与优先保留规则：
1. 若批次中包含标记为 `HEAD` 的提交，将其第一父链路标记为首要泳道消费者；
2. 如果存在待连往主干的泳道（`lane == 0`），非主干提交不占用 `Lane 0`，优先分配 `Lane >= 1`；
3. 保证 HEAD 主干连线从上至下贯穿最左侧。

- [ ] **Step 4: 运行测试以确认通过**

运行：`cargo test -p git_ui git_graph::test_lane_0_head_priority`  
预期：PASS。

- [ ] **Step 5: 提交更改**

```bash
git add crates/git_ui/src/git_graph.rs
git commit -m "git_graph: enforce Lane 0 priority for HEAD main trunk"
```

---

### Task 3: Canvas 渲染层全链路高亮与置灰 (Canvas Highlighting & Dimming)

**Files:**
- Modify: `crates/git_ui/src/git_graph.rs:1319-1350`, `crates/git_ui/src/git_graph.rs:3212-3480`, `crates/git_ui/src/git_graph.rs:1850-1940`
- Test: `crates/git_ui/src/git_graph.rs:5300-5380`

- [ ] **Step 1: 在 `GitGraph` 中添加 `active_lineage` 状态与交互事件**

在 `GitGraph` 结构体中添加字段：
```rust
active_lineage: Option<HashSet<Oid>>,
active_lineage_target: Option<Oid>,
```
提供方法：
```rust
fn set_active_lineage(&mut self, target: Option<Oid>, cx: &mut Context<Self>) {
    if self.active_lineage_target == target {
        return;
    }
    self.active_lineage_target = target;
    self.active_lineage = target.map(|sha| self.graph_data.find_lineage(sha));
    cx.notify();
}
```

- [ ] **Step 2: 在 `render_graph_canvas` 中实现节点与线条的高亮与置灰渲染**

修改 [`GitGraph::render_graph_canvas`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/git_graph.rs#L3265-L3430)：
1. 绘制 Commit 圆点时：
   ```rust
   let is_in_lineage = self.active_lineage.as_ref().map_or(true, |set| set.contains(&row.data.sha));
   let circle_color = if is_in_lineage {
       row_color
   } else {
       row_color.opacity(0.18)
   };
   draw_commit_circle(commit_x, row_y_center, circle_color, window);
   ```
2. 绘制 Commit 连线时：
   ```rust
   let is_line_in_lineage = self.active_lineage.as_ref().map_or(true, |set| {
       // 当起点和终点都在链路中时高亮
       set.contains(&line.child) && set.contains(&line.parent)
   });
   let line_color = if is_line_in_lineage {
       accent_color
   } else {
       accent_color.opacity(0.15)
   };
   ```

- [ ] **Step 3: 在表格行渲染中弱化非高亮项并绑定点击/悬停事件**

在 `render_table_rows` 中：
- 选中某行时调用 `set_active_lineage(Some(selected_sha), cx)`；
- 悬停在分支芯片（Ref Chip）或圆点时临时调用高亮；
- 按 `Escape` 键在 `cancel` 处理中清除 `set_active_lineage(None, cx)`。

- [ ] **Step 4: 编写并运行交互测试**

测试选中提交后 `active_lineage` 正确生成，按 `Escape` 恢复 `None`。  
运行：`cargo test -p git_ui git_graph`  
预期：PASS。

- [ ] **Step 5: 提交更改**

```bash
git add crates/git_ui/src/git_graph.rs
git commit -m "git_graph: implement lineage highlighting and canvas dimming"
```

---

### Task 4: 分支对比状态建模与 Ahead / Behind / Base 计算

**Files:**
- Modify: `crates/git_ui/src/git_graph.rs:1310-1420`
- Test: `crates/git_ui/src/git_graph.rs:4900-4980`

- [ ] **Step 1: 编写分支异同 Ahead / Behind 算法的单元测试**

在测试模块中新增 `test_branch_compare_ahead_behind`：

```rust
#[gpui::test]
fn test_branch_compare_ahead_behind() {
    let mut graph_data = GraphData::new(8);
    // Base: c1
    // Target branch (feature): c1 -> c2 -> c3
    // Base branch (main): c1 -> c4
    let c1 = Oid::from_str("1111111111111111111111111111111111111111").unwrap();
    let c2 = Oid::from_str("2222222222222222222222222222222222222222").unwrap();
    let c3 = Oid::from_str("3333333333333333333333333333333333333333").unwrap();
    let c4 = Oid::from_str("4444444444444444444444444444444444444444").unwrap();

    let commits = vec![
        Arc::new(InitialGraphCommitData {
            sha: c3,
            parents: smallvec![c2],
            ref_names: vec!["HEAD -> feature".into()],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c4,
            parents: smallvec![c1],
            ref_names: vec!["refs/heads/main".into()],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c2,
            parents: smallvec![c1],
            ref_names: vec![],
        }),
        Arc::new(InitialGraphCommitData {
            sha: c1,
            parents: smallvec![],
            ref_names: vec![],
        }),
    ];

    graph_data.add_commits(&commits);

    let compare_result = graph_data.compute_branch_compare(c4, c3);
    assert_eq!(compare_result.ahead_commits, HashSet::from_iter([c2, c3]));
    assert_eq!(compare_result.behind_commits, HashSet::from_iter([c4]));
    assert_eq!(compare_result.merge_base, Some(c1));
}
```

- [ ] **Step 2: 运行测试以确认失败**

运行：`cargo test -p git_ui git_graph::test_branch_compare_ahead_behind`  
预期：FAIL（方法未定义）。

- [ ] **Step 3: 实现 `BranchCompareState` 与 `compute_branch_compare`**

在 `crates/git_ui/src/git_graph.rs` 中定义：
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchCompareState {
    pub base_branch: SharedString,
    pub target_branch: SharedString,
    pub ahead_commits: HashSet<Oid>,
    pub behind_commits: HashSet<Oid>,
    pub merge_base: Option<Oid>,
    pub filter_to_compared_only: bool,
}

impl GraphData {
    pub fn compute_branch_compare(&self, base_sha: Oid, target_sha: Oid) -> BranchCompareResult {
        let base_ancestors = self.find_ancestors(base_sha);
        let target_ancestors = self.find_ancestors(target_sha);

        let ahead_commits: HashSet<Oid> = target_ancestors.difference(&base_ancestors).copied().collect();
        let behind_commits: HashSet<Oid> = base_ancestors.difference(&target_ancestors).copied().collect();
        let common: HashSet<Oid> = target_ancestors.intersection(&base_ancestors).copied().collect();

        // 找拓扑最新的共同祖先
        let merge_base = self.commits.iter().find(|c| common.contains(&c.data.sha)).map(|c| c.data.sha);

        BranchCompareResult {
            ahead_commits,
            behind_commits,
            merge_base,
        }
    }
}
```

- [ ] **Step 4: 运行测试以确认通过**

运行：`cargo test -p git_ui git_graph::test_branch_compare_ahead_behind`  
预期：PASS。

- [ ] **Step 5: 提交更改**

```bash
git add crates/git_ui/src/git_graph.rs
git commit -m "git_graph: implement branch compare state and ahead-behind computation"
```

---

### Task 5: 对比工具栏与提交列表徽章 (Compare Bar UI & Commit Badges)

**Files:**
- Modify: `crates/git_ui/src/git_graph.rs:2569-2650`, `crates/git_ui/src/git_graph.rs:1880-1940`
- Test: `crates/git_ui/src/git_graph.rs:5380-5450`

- [ ] **Step 1: 编写对比工具栏与徽章渲染的集成测试**

测试进入分支对比模式后：
1. 工具栏包含对比提示标签与 Ahead/Behind 数量指示器；
2. 处于 `ahead_commits` 的条目包含 `Ahead` 徽章元素；
3. 处于 `behind_commits` 的条目包含 `Behind` 徽章元素。

- [ ] **Step 2: 运行测试以确认失败**

运行：`cargo test -p git_ui git_graph`  
预期：FAIL。

- [ ] **Step 3: 实现 `render_compare_bar` 与表格徽章渲染**

1. 在 `render_search_bar` 下方渲染醒目的分支对比条：
   - 绿色色块展示 `Ahead: +N`
   - 蓝色/红色色块展示 `Behind: -M`
   - 包含复选框 `[☑ 仅看相关提交]`（联动 `filter_to_compared_only`）
   - 包含退出按钮 `[✕ 退出对比]`
2. 在 `render_table_rows` 中渲染条目时，根据 `branch_compare_state` 前置添加 `Chip`：
   - `Ahead`：绿色微弱背景的 Chip；
   - `Behind`：蓝色微弱背景的 Chip；
   - `Base`：紫色微弱背景的 Chip；
3. 若 `filter_to_compared_only` 为 true，过滤 `rows` 仅保留两分支相关提交。

- [ ] **Step 4: 运行测试确认通过**

运行：`cargo test -p git_ui git_graph`  
预期：PASS。

- [ ] **Step 5: 提交更改**

```bash
git add crates/git_ui/src/git_graph.rs
git commit -m "git_graph: add compare toolbar and Ahead/Behind commit badges"
```

---

### Task 6: 详情面板“两分支总差异”支持 (Overall Branch Diff in Detail Panel)

**Files:**
- Modify: `crates/git_ui/src/git_graph.rs:2700-2950`, `crates/git_ui/src/commit_view.rs`
- Test: `crates/git_ui/src/git_graph.rs:5450-5520`

- [ ] **Step 1: 编写分支总差异加载与展示的测试用例**

验证当激活对比模式并切换到 `两分支总差异` 时，详情面板请求 `git diff base_sha...target_sha` 并展示聚合文件变更。

- [ ] **Step 2: 运行测试以确认失败**

运行：`cargo test -p git_ui git_graph`  
预期：FAIL。

- [ ] **Step 3: 在详情面板中实现两分支汇总 Diff 的异步加载与展示**

在 `GitGraph` 详情面板渲染中：
1. 顶部提供选项卡：`[当前选中提交]` | `[两分支总差异]`；
2. 在后台通过 `git_store` 执行 `diff_commits(base_sha, target_sha)`；
3. 将返回的文件修改列表和多缓冲 Diff 赋给现有 `solo_diff_view` / `multi_diff_view` 组件展示。

- [ ] **Step 4: 运行测试确认通过**

运行：`cargo test -p git_ui git_graph`  
预期：PASS。

- [ ] **Step 5: 提交更改**

```bash
git add crates/git_ui/src/git_graph.rs crates/git_ui/src/commit_view.rs
git commit -m "git_graph: support overall branch diff in commit detail panel"
```

---

### Task 7: 分支右键入口、Action 定义与本地化 (Context Menu & Localization)

**Files:**
- Modify: `crates/git_ui/src/git_manager/sections/branches.rs:400-500`
- Modify: `crates/git_ui/src/commit_context_menu.rs:40-100`
- Modify: `crates/settings_content/src/settings_content.rs`
- Test: `crates/git_ui/src/git_manager/sections/branches.rs`

- [ ] **Step 1: 定义 `CompareBranchWithCurrent` Action**

在 `crates/git_ui/src/git_ui.rs` 或 `git_graph.rs` 中：
```rust
#[derive(Clone, PartialEq, Action)]
pub struct CompareBranchWithCurrent {
    pub base_branch: SharedString,
}
```

- [ ] **Step 2: 在 Git Manager 分支列表右键菜单中增加入口**

在 [`crates/git_ui/src/git_manager/sections/branches.rs`](file:///home/noah/Codes/PC/Zed/crates/git_ui/src/git_manager/sections/branches.rs) 的右键菜单构建中：
- 为所有非 HEAD 分支添加菜单项：`"与当前分支对比..."` (`translate_ui("Compare with Current Branch...", cx)`)；
- 点击时打开或复用 Git Graph，并触发 `enter_branch_compare(branch_name, head_name, cx)`。

- [ ] **Step 3: 在 `commit_context_menu.rs` 分支标签右键中增加入口**

支持右键任意分支标签选择“与当前分支对比”。

- [ ] **Step 4: 补充中文汉化翻译**

在 [`crates/settings_content/src/settings_content.rs`](file:///home/noah/Codes/PC/Zed/crates/settings_content/src/settings_content.rs) 中添加以下中文翻译条目：
- `"Compare with Current Branch..."` $\rightarrow$ `"与当前分支对比..."`
- `"Exit Compare Mode"` $\rightarrow$ `"退出对比模式"`
- `"Filter to Compared Commits"` $\rightarrow$ `"仅显示对比提交"`
- `"Branch Total Diff"` $\rightarrow$ `"两分支总差异"`
- `"Ahead"` $\rightarrow$ `"领先 (Ahead)"`
- `"Behind"` $\rightarrow$ `"落后 (Behind)"`
- `"Base"` $\rightarrow$ `"分叉点"`

- [ ] **Step 5: 运行相关测试确认通过**

运行：`cargo test -p git_ui`  
预期：PASS。

- [ ] **Step 6: 提交更改**

```bash
git add crates/git_ui/ crates/settings_content/
git commit -m "git_ui: add compare branch context menu entries and localization"
```

---

### Task 8: 全面回归测试与代码规范检查 (Full Regression & Clippy)

**Files:**
- All touched files

- [ ] **Step 1: 运行所有相关单元与集成测试**

```bash
cargo test -p git_ui git_graph
cargo test -p git_ui git_manager
```
预期：所有测试全部 PASS。

- [ ] **Step 2: 运行 Clippy 静态检查**

```bash
./script/clippy -p git_ui -p settings_content
```
预期：0 warnings, 0 errors。

- [ ] **Step 3: 最终提交与记录**

```bash
git commit --allow-empty -m "chore: complete git graph branch visualization and comparison verification"
```
