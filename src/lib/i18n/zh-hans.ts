// The TS dictionaries ARE the i18n source (ADR-0019: the resx pipeline is retired).
import type { en } from './en'

export const zhHans: Record<keyof typeof en, string> = {
  "About_Author": "小明 · XiaomingLab",
  "About_AuthorTagline": "同一件事做两次，就写个工具",
  "About_Back": "返回",
  "About_BackToAbout": "← 关于",
  "About_Changelog": "更新日志",
  "About_CheckUpdate": "检查更新",
  "About_Chip_Local": "本地运行",
  "About_Chip_NoAccount": "无账号",
  "About_Chip_NoTelemetry": "无遥测",
  "About_Chip_OpenSource": "免费开源",
  "About_Chip_Reversible": "全程可还原",
  "About_Feedback": "反馈建议",
  "About_Footer": "© 2026 XiaomingLab · Windows 10 / 11",
  "About_Homepage": "xiaominglab.com",
  "About_Link_Bilibili": "哔哩哔哩",
  "About_Link_Douyin": "抖音",
  "About_Link_GitHub": "GitHub",
  "About_Link_Home": "个人主页",
  "About_Link_X": "X",
  "About_Ok": "好",
  "About_RepoTitle": "开源地址 · 欢迎 Star",
  "About_RepoUrl": "github.com/nicepkg/deskmakeover",
  "About_Slogan": "让 Windows 回到它本该的样子",
  "About_Tagline": "让 Windows 回到它本该的样子。",
  "About_Trust": "全程本地运行 · 不联网 · 不上传任何数据",
  "About_VersionFormat": "版本 {0}",
  "About_VersionLine": "DeskMakeover · v1.0.0（2026.07）",
  "AppTitle": "桌面美颜",
  "Appearance_Custom": "自定义",
  "Applying": "正在美化你的桌面…",
  "ArrowGate_Body": "这么丑的东西你都能忍、都能喜欢，那你一定也不在乎多等这六十秒。", // PENDING-RESX (owner decree, 60s re-affirmed 2026-07-09)
  "ArrowGate_Cancel": "点错了，返回", // PENDING-RESX
  "ArrowGate_Confirm": "我确定，丑我认了", // PENDING-RESX
  "ArrowGate_Stare1": "好好看看它。", // PENDING-RESX
  "ArrowGate_Stare2": "还在看？它不会变好看的。", // PENDING-RESX
  "ArrowGate_Stare3": "行，你是真的爱它。", // PENDING-RESX
  "ArrowGate_Title": "你认真的？", // PENDING-RESX
  "ArrowGate_Wait": "{0} 秒后可确认", // PENDING-RESX
  "ArrowRestore_Title": "恢复系统快捷方式箭头？", // PENDING-RESX (arrow-restore panel 2026-07-11)
  "ArrowRestore_Body": "所有快捷方式会重新显示 Windows 自带的箭头，包括你在桌面美化过的图标。你的形状和配色不会改变。恢复时系统会弹出一次权限确认。", // PENDING-RESX
  "ArrowRestore_Confirm": "恢复箭头", // PENDING-RESX
  "ArrowRestore_Cancel": "取消", // PENDING-RESX
  "Axis_Color": "配色",
  "Axis_Dist": "快捷方式标识",
  "Axis_Filter": "滤镜",
  "Axis_Kind": "参与美化的类型",
  "Axis_Shape": "外形",
  "BadgeCleanWarning": "",
  "BadgeStyle_Arrow": "箭头",
  "BadgeStyle_Chamfer": "切角",
  "BadgeStyle_Dock": "停靠基座",
  "BadgeStyle_Fold": "翻页角",
  "BadgeStyle_Gem": "宝石",
  "BadgeStyle_Matte": "装裱衬边",
  "BadgeStyle_Stacked": "叠影卡片",
  "Badge_Clean": "去除",
  "Badge_Keep": "保留原样",
  "Badge_Refined": "美化",
  "Badge_RowLabel": "快捷方式",
  "Canvas_Placeholder": "桌面预览",
  "Canvas_Refresh_Tip": "重新读取桌面（图标、排列与壁纸）",
  "Cap_Close": "关闭",
  "Cap_Maximize": "最大化",
  "Cap_Minimize": "最小化",
  "Cap_Restore": "向下还原",
  "Changelog_Title": "更新日志",
  "Clarity_Off": "关",
  "Clarity_Soft": "柔和",
  "Clarity_Strong": "强",
  "Color_Bw": "黑白",
  "Color_Field": "满彩", // PENDING-RESX (ADR-0016 默认模式)
  "Field_Quiet": "柔和", // PENDING-RESX
  "Field_Vivid": "鲜明", // PENDING-RESX
  "Type_FollowGlobal": "跟随全局", // PENDING-RESX (ADR-0017)
  "Type_Custom": "自定义", // PENDING-RESX (ADR-0017)
  "Type_ResetFollow": "恢复跟随全局", // PENDING-RESX (ADR-0017)
  "Type_ResetAll": "全部重置", // PENDING-RESX (ADR-0017)
  "Type_PlateAuto": "自动底板", // PENDING-RESX (ADR-0017)
  "Type_Plate": "底板", // PENDING-RESX (ADR-0017)
  "Subject_Label": "主体", // PENDING-RESX (ADR-0018 两轴)
  "Plate_Label": "底板", // PENDING-RESX (ADR-0018 两轴)
  "Subject_Orig": "原彩", // PENDING-RESX (ADR-0018)
  "Plate_Auto": "随图标", // PENDING-RESX (ADR-0018)
  "Plate_Faithful": "本色", // PENDING-RESX (ADR-0018)
  "Plate_White": "白", // PENDING-RESX (ADR-0018)
  "Plate_NeedShape": "选一个形状后可换底色", // PENDING-RESX (ADR-0018)
  "Shortcut_UniformShape": "快捷方式形状", // PENDING-RESX (ADR-0017, 无=不统一)
  "Shortcut_ShapeGhost": "快捷方式已统一形状，覆盖各类型形状", // PENDING-RESX (ADR-0017)
  "Color_Mono": "单色",
  "Color_Orig": "原彩",
  "ComboLabel": "风格",
  "Combo_Apple": "苹果极简",
  "Combo_Bw": "纯净黑白",
  "Combo_Candy": "糖果彩",
  "Combo_Wallpaper": "壁纸同色",
  "Compare_After": "美化后",
  "Compare_Before": "美化前",
  "Compare_Held": "原来的样子",
  "Compare_Idle": "按住对比原样 · 空格",
  "Compare_Short": "对比",
  "ComingSoon": "即将推出", // PENDING-RESX (v3 filter roadmap slot)
  "ConsentAgree": "好，开始美化",
  "ConsentArrow": "应用美化时，我们会隐藏 Windows 自带的快捷方式小箭头，改由 DeskMakeover 统一绘制，让图标更清爽。这项改动对整台电脑生效，也会影响桌面以外、以及本机其他账户的快捷方式；随时可以在设置里一键恢复。", // PENDING-RESX (arrow-restore panel 2026-07-11)
  "ConsentCancel": "再想想",
  "ConsentNot": "不会发生：不删除、不移动、不修改你的任何文件",
  "ConsentTitle": "开始前，说清楚三件事",
  "ConsentUac": "需要一次管理员授权，Windows 会弹出确认框",
  "ConsentWhatFormat": "会发生：美化 {0} 个图标的外观，并把快捷方式小箭头换成精致标记",
  "Corner_Square": "直角",
  "Cta_Apply": "一键美化",
  "Cta_Scanning": "正在扫描…",
  "Cta_Synced": "✓ 已与桌面同步",
  "Cta_Update": "更新桌面",
  "Cta_Working": "正在应用…",
  "Custom_Badge": "自定义中",
  "Dist_Keep": "默认箭头", // PENDING-RESX: was 经典箭头 (owner: name the native artifact)
  "Dist_Mark": "美化标识",
  "Dist_None": "无标识",
  "DoneArrow": "系统快捷方式箭头已隐藏，桌面更清爽了。想找回小箭头，到设置里可以一键恢复。", // PENDING-RESX (arrow-restore panel 2026-07-11)
  "DoneHeadline": "✨ 好了，你的桌面焕然一新",
  "Done_GoOrganize": "去桌面整理",
  "Done_LastStep": "最后一步：把图标拖进分区，Windows 网格会自动对齐。",
  "Drawer_AboutRow": "关于桌面美颜",
  "Drawer_AboutRowDesc": "v1.0 · 让 Windows 回到它本该的样子",
  "Drawer_Appearance": "外观主题",
  "Drawer_ChangelogRow": "更新日志",
  "Drawer_CheckUpdate": "检查更新",
  "Drawer_Comparison": "前后对比图",
  "Drawer_ComparisonDesc": "生成一张可分享的对比图片",
  "Drawer_Export": "导出",
  "Drawer_Feedback": "联系反馈",
  "Drawer_KeepUp": "新图标自动美化",
  "Drawer_KeepUpDesc": "桌面出现新图标时按当前风格处理",
  "Drawer_Snapshot": "还原快照",
  "Drawer_SnapshotNone": "暂无 · 应用时自动创建",
  "Drawer_SnapshotSaved": "1 份 · 刚刚自动保存",
  "Drawer_Title": "设置",
  "EmptyStateLine": "你的桌面，即将焕然一新",
  "ErrorDetails": "技术细节",
  "ErrorHeadline": "出了点问题",
  "ErrorNextStep": "可以稍后再试；如果反复出现，请在设置里导出诊断信息",
  "ErrorNothingChanged": "你的桌面没有被改动",
  "ErrorRolledBack": "已自动撤销这次改动，桌面回到了原样",
  "Filter_Glass": "玻璃",
  "Filter_Gloss": "光泽", // PENDING-RESX (v3 coming-soon filter slot)
  "Filter_None": "无",
  "Filter_Pixel": "像素",
  "Filter_Sticker": "贴纸",
  "Glyph_Arrow": "箭头",
  "Glyph_Dot": "纯箭头",
  "Glyph_Fold": "描边箭头",
  "GoSeeDesktop": "去看看桌面",
  "Gradient_Bottom": "底部",
  "Gradient_Custom": "自定义", // PENDING-RESX (v3 dial↔direction sync)
  "Gradient_Left": "左侧",
  "Gradient_Right": "右侧",
  "Gradient_Top": "顶部",
  "Gradient_Vignette": "四角",
  "Hero_CleanStatus": "已美化 {0} 个图标 · 快照已保存",
  "Hero_DirtyStatus": "已美化 {0} 个图标 · 有新样式待应用",
  "Hero_ReadyStatus": "可以美化 {0} 个图标 · 全程可还原",
  "Hero_Scanning": "正在扫描桌面…",
  "Hero_Title": "你的桌面，即将焕然一新",
  "Hero_TitleClean": "已经焕然一新",
  "Hero_TitleDirty": "有新的样式待应用",
  "History_BackToInitial": "回到最初",
  "History_Current": "当前",
  "History_GoTo": "回到此版",
  "History_Header": "版本历史 · 保留最近 10 版",
  "History_Initial": "最初",
  "History_Custom": "自定义", // PENDING-RESX
  "History_InitialDesc": "Windows 原生桌面",
  "History_Redo": "重做",
  "History_Undo": "撤销",
  "Icons_ApplyProgress": "正在写入桌面 {0}/{1}", // PENDING-RESX
  "Icons_ClearExceptions": "清除所有例外", // PENDING-RESX
  "KindBucket_App": "程序",
  "KindBucket_Folder": "文件夹",
  "KindBucket_File": "文档",
  "KindBucket_System": "系统",
  "Icons_KeepAllKind": "所有{0}不参与美化",
  "Icons_ReincludeKind": "让{0}参与美化",
  "Icons_KeptCount": "保留原样 {0} 个",
  "Icons_ClearKept": "清除",
  "Icons_ReincludeCancel": "参与",
  "Icons_ExceptionCount": "例外 {0} 个", // PENDING-RESX
  "Icons_MenuIconSize": "图标大小", // PENDING-RESX
  "Icons_MenuRefresh": "刷新", // PENDING-RESX
  "Icons_PeekHint": "按住查看原图标", // PENDING-RESX
  "Icons_SizeHonesty": "应用后位置由 Windows 重新排列", // PENDING-RESX
  "Icons_Unstyleable": "此图标由 Windows 管理，无法修改", // PENDING-RESX
  "Ink_Auto": "自动",
  "Ink_Black": "黑",
  "Ink_White": "白",
  "ItemCountFormat": "{0} 个项目",
  "KeepUpNote": "新图标会在打开应用时自动跟上",
  "Keymap_Compare": "对比原样",
  "Keymap_CompareKey": "按住空格",
  "Keymap_DeleteZone": "删除分区",
  "Keymap_DeleteZoneKey": "Del",
  "Keymap_Deselect": "取消选中分区", // PENDING-RESX (v3 keymap by page)
  "Keymap_DeselectKey": "Esc", // PENDING-RESX
  "Keymap_Modules": "切换模块",
  "Keymap_ModulesKey": "Ctrl+1/2/3",
  "Keymap_NewZone": "新建分区",
  "Keymap_NewZoneKey": "空白处拖拽", // PENDING-RESX: was 画布拖拽 (page-scoped now)
  "Keymap_Open": "快捷键说明",
  "Keymap_Pan": "平移画布", // PENDING-RESX (v3 keymap by page)
  "Keymap_PanIconsKey": "空白处拖拽", // PENDING-RESX
  "Keymap_PanPaperKey": "中键拖拽", // PENDING-RESX
  "Keymap_Redo": "重做", // PENDING-RESX (v3 keymap by page)
  "Keymap_RedoKey": "Ctrl+Shift+Z", // PENDING-RESX
  "Keymap_Title": "快捷键",
  "Keymap_Undo": "撤销",
  "Keymap_UndoKey": "Ctrl+Z",
  "Keymap_Zoom": "缩放画布", // PENDING-RESX (v3 keymap by page)
  "Keymap_ZoomKey": "Ctrl+滚轮", // PENDING-RESX (owner: keyboard+mouse only, no touch gestures)
  "KeymapSec_General": "通用", // PENDING-RESX (v3 keymap by page)
  "KeymapSec_Icons": "图标", // PENDING-RESX
  "KeymapSec_Paper": "壁纸", // PENDING-RESX
  "Kind_AppxShortcut": "应用商店应用",
  "Kind_Folder": "文件夹",
  "Kind_Other": "其他",
  "Kind_RecycleBin": "回收站",
  "Kind_RegularFile": "文件",
  "Kind_Shortcut": "快捷方式",
  "Kind_UrlShortcut": "网页快捷方式",
  "Language_English": "English",
  "Language_System": "跟随系统",
  "Language_ZhHans": "简体中文",
  "Link_Compare": "对比图",
  "Link_Compare_Tip": "保存前后对比图",
  "Link_History": "历史",
  "Link_History_Tip": "版本历史",
  "Link_Prev": "上一版",
  "Link_Prev_Tip": "回到上一版",
  "Link_Restore": "还原",
  "Link_Restore_Tip": "还原到 Windows 原生桌面",
  "MarkColor_Auto": "自动",
  "MarkColor_Label": "标识配色",
  "Mark_Arc": "珐琅光弧",
  "Mark_ArrowHint": "这些角标都是画上去的。应用后，系统自带的小箭头会隐藏，随时可以在设置里恢复。", // PENDING-RESX (arrow-restore panel 2026-07-11)
  "Mark_Halo": "光环",
  "Mark_Shadow": "投影",
  "Mark_Fold": "卷角",
  "Mark_Ring": "细描边",
  "Mark_Satin": "缎光角",
  "Material_Frost": "磨砂玻璃",
  "Material_Halo": "柔光晕影",
  "Material_Luminous": "晨光玻璃",
  "Material_Outline": "描边卡片",
  "Material_Solid": "实色卡片",
  "Menu_Follow": "跟随全局样式",
  "Menu_FollowToast": "「{0}」已跟随全局样式",
  "Menu_Keep": "保留原样",
  "Menu_KeptToast": "「{0}」将保留原样",
  "Menu_TintHeader": "单独配色",
  "Menu_TintToast": "已为「{0}」单独配色",
  "Menu_UnkeptToast": "「{0}」恢复跟随全局",
  "MirrorReadyFormat": "开启后，{0} 个桌面图标会统一成一种干净外观",
  "MirrorSubtitle": "{0} 个图标",
  "MirrorSubtitleSkipped": "{0} 个图标 · {1} 个保持原样",
  "MirrorTitle": "你的桌面",
  "Palette_Button": "调色盘",
  "PlateColor_Auto": "自动",
  "ColorTab_Bg": "背景",
  "ColorTab_Fg": "前景",
  "Mono_Flat": "纯色",
  "Mono_Tonal": "渐变",
  "Panel_IconsTitle": "美化图标",
  "Panel_PaperTitle": "美化桌面壁纸",
  "Panel_Placeholder": "控制面板",
  "Panel_SettingsTitle": "设置",
  "Paper_AddZone": "+ 添加分区",
  "Paper_Advanced": "高级",
  "Paper_Angle": "角度",
  "Paper_Clarity": "壁纸压暗",
  "Paper_Coach": "分区会被画进壁纸,图标不会自动跑进去。把图标拖到框里,Windows 网格会帮它们站整齐。原壁纸已自动备份,随时一键换回。",
  "Paper_CoachOk": "知道了",
  "Paper_Corner": "圆角",
  "Paper_Cta_Apply": "应用到壁纸",
  "Paper_Cta_Synced": "✓ 已与桌面同步",
  "Paper_Cta_Update": "更新壁纸",
  "Paper_Cta_Working": "正在合成…",
  "Paper_Dim": "压暗强度",
  "Paper_DropHint": "松手，用这张图设计",
  "Paper_DropReject": "只支持图片文件",
  "Paper_EmptyDrawHint": "或在壁纸上拖一个框，自己划分区",
  "Paper_EmptyImportHint": "也可以导入自己的图片",
  "Paper_EmptyLead": "选一套布局开始",
  "Paper_Export": "导出图片",
  "Paper_Export_Tip": "保存成图片，发给别人也能看",
  "Paper_FillColor": "填充颜色",
  "Paper_FillOpacity": "不透明度",
  "Paper_Footer": "分区是画在壁纸上的底板 · 图标要你自己拖进去 · 原壁纸已自动备份",
  "PaperHow_Title": "它如何工作", // PENDING-RESX (v3 how-it-works card)
  "PaperHow_Zones": "分区会直接画进壁纸图片里", // PENDING-RESX (v3 how-it-works card)
  "PaperHow_Icons": "图标要你自己拖进分区摆放", // PENDING-RESX (v3 how-it-works card)
  "PaperHow_Backup": "原壁纸自动备份,随时一键还原", // PENDING-RESX (v3 how-it-works card)
  "Paper_Gradient": "渐变方向",
  "Paper_Hero": "给壁纸分个区",
  "Paper_HeroApplied": "桌面已经很整齐了",
  "Paper_Import": "导入壁纸",
  "Paper_ImportCancel": "取消导入",
  "Paper_ImportCancel_Tip": "换回当前桌面壁纸",
  "Paper_Import_Tip": "用一张自己的图来设计",
  "Paper_Mismatch": "桌面环境变了,分区可能错位",
  "Paper_PaleHint": "壁纸偏亮，压暗后图标更清楚",
  "Paper_Regenerate": "重新合成",
  "Paper_ReplaceConfirm": "换成这套布局？当前分区会被替换。",
  "Paper_Restore": "换回我的壁纸",
  "Paper_ScrimLabel": "压暗颜色",
  "Paper_SourceImported": "正在设计导入的图片",
  "Paper_StatusApplied": "已应用 · 原壁纸已备份",
  "Paper_StatusDirty": "有新改动待应用",
  "Paper_StatusIdle": "预览即所得 · 放心试", // PENDING-RESX (v3 short status, never truncates)
  "Paper_StatusWorking": "正在合成壁纸…",
  "Paper_TitleLabel": "标题",
  "Paper_ZoneStyle": "分区样式",
  "Paper_Zones": "分区",
  "PlanNeedsSnapshot": "生成应用计划前，请先保存快照。",
  "PlanReadyFormat": "应用计划已生成：{0} 个安全预览步骤。尚未修改桌面。",
  "Preset_Apply": "应用此布局",
  "Preset_Gallery": "预设布局",
  "Preset_MinimalDuo": "极简双区",
  "Preset_Quadrants": "四象限",
  "Preset_SideRail": "左栏收纳",
  "Preset_Workbench": "工作台",
  "Preset_faithful": "原彩保真", // PENDING-RESX (ADR-0016 lineup)
  "Preset_faithful_Desc": "忠实还原每个图标的底色", // PENDING-RESX
  "Preset_field": "满彩", // PENDING-RESX (ADR-0016 default)
  "Preset_spectrum": "满彩", // PENDING-RESX (预设v2)
  "Preset_spectrum_Desc": "满城彩色，各归其位", // PENDING-RESX
  "Preset_stationery": "暖纸文具", // PENDING-RESX
  "Preset_stationery_Desc": "一桌牛皮纸与马尼拉信封", // PENDING-RESX
  "Preset_glass": "澄玻璃", // PENDING-RESX
  "Preset_glass_Desc": "会呼吸的液态玻璃", // PENDING-RESX
  "Preset_pebble": "卵石花园", // PENDING-RESX
  "Preset_pebble_Desc": "一桌温润鹅卵石，没有尖角", // PENDING-RESX
  "Preset_ink": "水墨宣", // PENDING-RESX
  "Preset_ink_Desc": "一屏黑白见筋骨", // PENDING-RESX
  "Preset_white": "极简白", // PENDING-RESX
  "Preset_white_Desc": "白纸一张，只理形状", // PENDING-RESX
  "Preset_ascast": "本色", // PENDING-RESX
  "Preset_ascast_Desc": "原样保真，只理齐轮廓", // PENDING-RESX
  "Preset_MoreN": "更多风格 +{0}", // PENDING-RESX
  "Preset_Collapse": "收起", // PENDING-RESX
  "Preset_field_Desc": "统一外形 · 各自品牌色", // PENDING-RESX
  "Preset_minimal": "极简白", // PENDING-RESX
  "Preset_minimal_Desc": "安静的白色瓷砖", // PENDING-RESX
  "Preset_quiet": "安静", // PENDING-RESX
  "Preset_quiet_Desc": "柔彩底板 · 色相各异", // PENDING-RESX
  "PreviewApplyPlan": "预览应用计划",
  "PreviewScanComplete": "预览扫描完成。尚未修改桌面。",
  "ProductName": "桌面美颜",
  "Rail_Icons": "图标",
  "Rail_Paper": "壁纸",
  "Rail_Settings": "设置",
  "ReadyToScan": "准备扫描你的桌面。",
  "Reason_RecycleBin": "这是系统回收站，Windows 不允许修改它的图标",
  "Reason_RegularFile": "普通文件保持原样，可在设置里开启文件美化",
  "Reason_Unsupported": "这个项目暂时不支持美化，它不会被改动",
  "RefreshNotice": "桌面会闪一下，大约 2 秒，打开的窗口和文件不会受影响",
  "RestoreConfirm": "确定变回原来的样子吗？",
  "Restored": "已还原系统默认 · 无残留",
  "Restoring": "正在还原…",
  "Restyling": "更新中…",
  "RetryBadge": "再试一次小箭头美化",
  "SaveComparison": "保存对比图",
  "SaveSnapshot": "保存快照",
  "ScanDesktop": "扫描桌面",
  "ScanFailedPrefix": "扫描失败。没有修改任何内容。",
  "Scanning": "正在看看你的桌面…",
  "Scrim_Dark": "深色",
  "Scrim_Light": "浅色",
  "Scrim_Tint": "壁纸色",
  "Section_Custom": "自定义",
  "Section_Style": "风格",
  "SettingsTitle": "设置",
  "Settings_AboutHelp": "关于与帮助",
  "Settings_Appearance": "外观",
  "Settings_AppearanceDesc": "默认跟随 Windows。只有想让应用单独呈现时再改。",
  "Settings_ArrowRestore": "快捷方式箭头", // PENDING-RESX (arrow-restore panel 2026-07-11)
  "Settings_ArrowStatusHidden": "当前：已隐藏，由 DeskMakeover 统一绘制", // PENDING-RESX
  "Settings_ArrowStatusNative": "当前：Windows 默认", // PENDING-RESX
  "Settings_ArrowRestoreAction": "恢复系统箭头", // PENDING-RESX
  "Settings_ArrowConstraint": "系统限制只能整机切换。", // PENDING-RESX
  "Settings_Backup": "备份位置",
  "Settings_Badge": "区分快捷方式",
  "Settings_BadgeDesc": "默认给快捷方式加一道柔和的左下标记来区分；也可保留原箭头或完全去除",
  "Settings_Diagnostics": "诊断信息",
  "Settings_Export": "导出",
  "Settings_Diag": "问题诊断", // PENDING-RESX (v3 error reporting)
  "Settings_DiagDesc": "复制错误日志，或带上环境信息向作者报告问题", // PENDING-RESX (v3 error reporting)
  "Diag_CopyLog": "复制错误日志", // PENDING-RESX (v3 error reporting)
  "Diag_Report": "去 GitHub 报告", // PENDING-RESX (v3 error reporting)
  "Diag_Email": "发邮件给作者", // PENDING-RESX (v3 error reporting)
  "Diag_Copied": "错误日志已复制", // PENDING-RESX (v3 error reporting)
  "Diag_CopyFailed": "复制失败,请手动选择复制", // PENDING-RESX (v3 error reporting)
  "Settings_ExportCompare": "保存前后对比图",
  "Settings_Feedback": "联系反馈",
  "Settings_General": "通用",
  "Settings_KeepUp": "新图标自动跟上",
  "Settings_KeepUpDesc": "打开应用时，自动美化新添加的图标",
  "Settings_Language": "语言",
  "Settings_LocalData": "本地数据",
  "Settings_LocalDataDesc": "还原快照和对比图都留在这台电脑上。",
  "Settings_MarkColor": "标记颜色",
  "Settings_OpenDataFolder": "打开数据文件夹",
  "Settings_OpenFolder": "打开文件夹",
  "Settings_PageSubtitle": "偏好、本地备份、更新与产品信息都放在这里。",
  "Settings_Theme": "主题",
  "ShapeLabel": "外形",
  "Shape_Apple": "苹果",
  "Shape_Bookmark": "书签",
  "Shape_Folder": "文件夹", // PENDING-RESX (ADR-0017)
  "Shape_Circle": "纯圆",
  "Shape_Diamond": "菱形",
  "Shape_Flower": "花瓣",
  "Shape_Lemon": "柠檬",
  "Shape_More": "更多", // PENDING-RESX (v3 inspector)
  "Shape_None": "无",
  "Shape_Pebble": "卵石",
  "Shape_Samsung": "三星",
  "Shape_Teardrop": "水滴",
  "Shape_Tile": "方块",
  "Size_Big": "大",
  "Size_Mid": "中",
  "Size_Small": "小",
  "SkippedDetailsFormat": "{0} 个项目会保持原样（点击查看原因）",
  "Slogan": "一键美颜你的 Windows 桌面，随时完整还原。",
  "SnapshotSavedFormat": "快照已保存。还原数据位于 {0}。",
  "StandingPromise": "只美化图标外观，不动你的文件 · 全程本地不联网 · 关闭即完整还原",
  "State_Error": "读取失败",
  "State_Other": "未知",
  "State_PreviewOnly": "暂不处理",
  "State_Ready": "可美化",
  "State_RequiresConsent": "需要确认",
  "State_Unsupported": "保持原样",
  "StyleLabel": "配色",
  "Swatch_Amber": "琥珀",
  "Swatch_Black": "纯黑",
  "Swatch_Coral": "品牌珊瑚",
  "Swatch_Teal": "湖水",
  "Swatch_WallPrimary": "壁纸主色（自动提取）",
  "Swatch_WallSecondary": "壁纸辅色（自动提取）",
  "Swatch_White": "纯白",
  "SwitchOffAction": "关闭并完整还原",
  "SwitchOnAction": "开启桌面美颜",
  "SwitchOnStateFormat": "美颜已开启 · {0} 个图标已统一",
  "Theme_Dark": "深色",
  "Theme_Light": "浅色",
  "Theme_System": "跟随系统",
  "TitleFont_Default": "默认字体",
  "TitleStyle_Bar": "顶栏标题",
  "TitleStyle_Bare": "净色标题",
  "TitleStyle_Chip": "胶囊标签",
  "TitleStyle_Tab": "折角页签",
  "ToastApplied": "美化完成，处理了 {0} 个图标",
  "ToastCaughtUp": "已自动美化 {0} 个新增图标",
  "ToastRestored": "已还原系统默认，无残留",
  "ToastSaved": "对比图已保存到图片文件夹",
  "Toast_Applied": "美化完成 · 已保存还原快照",
  "Toast_AppliedNoOverlay": "图标已美化 · 隐藏箭头一步已跳过（未授权）",
  "Toast_ApplyFailed": "美化未完成 · 桌面没有改动，一切安好",
  "Toast_ArrowRestored": "已恢复系统箭头", // PENDING-RESX (arrow-restore panel 2026-07-11)
  "Toast_BackTo": "已回到：{0}",
  "Toast_CompareFailed": "对比图保存失败，请重试",
  "Toast_CompareSaved": "对比图已保存：{0}",
  "Toast_ComparisonSaved": "对比图已保存到桌面",
  "Toast_ImportFailed": "这张图打不开，换一张试试",
  "Toast_PaperApplied": "壁纸已应用 · 原壁纸已备份",
  "Toast_PaperAppliedSlideshow": "壁纸已应用 · 幻灯片已暂停,换回时恢复",
  "Toast_PaperApplyFailed": "壁纸应用失败,桌面未改动",
  "Toast_PaperExportFailed": "导出失败，请重试",
  "Toast_PaperExported": "壁纸已导出：{0}",
  "Toast_PaperRestoreFailed": "换回失败,备份仍在,可重试",
  "Toast_PaperRestored": "已换回你原来的壁纸",
  "Toast_ExceptionsCleared": "已清除所有例外", // PENDING-RESX
  "Toast_Refreshed": "已重新读取桌面",
  "Toast_RestoreFailed": "未能完全还原 · 已保留可重试，原始桌面数据安全",
  "Toast_Restored": "已还原 · 桌面回到原来的样子",
  "Toast_SnapshotExported": "快照已导出",
  "Toast_UpToDate": "已是最新版本 v1.0",
  "Tune_Header": "在当前风格上微调",
  "Tune_Label": "调整",
  "UacDeclined": "已跳过小箭头美化（未授权）。其它美化已照常完成，随时可以再试一次。",
  "Updating_Cue": "正在更新预览…",
  "VersionChip": "v1.0",
  "Welcome_BluffBody": "还在？就知道你舍不得。嘴上不承认，手很诚实。你是今天第 {0} 个嘴硬的，前面那些，最后都乖乖抄完了下面这句话。", // PENDING-RESX (welcome gate)
  "Welcome_BluffCta": "提交认错书", // PENDING-RESX (typed-recant gate)
  "Welcome_BluffCtaLocked": "抄完才能进", // PENDING-RESX
  "Welcome_Confession": "我错了，Windows桌面的图标和快捷方式角标真的很丑", // PENDING-RESX (owner verbatim, typed by hand)
  "Welcome_Continue": "继续", // PENDING-RESX
  "Welcome_EnterCta": "开始美化", // PENDING-RESX
  "Welcome_CopyPrompt": "照抄这句话，一字不差：", // PENDING-RESX
  "Welcome_Gate2No": "挺整齐的，没觉得乱", // PENDING-RESX
  "Welcome_Gate2Question": "那第三方应用的图标呢，摆在桌面上整齐吗？", // PENDING-RESX
  "Welcome_Gate2Yes": "乱七八糟，看着难受", // PENDING-RESX
  "Welcome_GateNo": "挺好的，没觉得丑", // PENDING-RESX
  "Welcome_GateQuestion": "你觉得 Windows 原生的快捷方式角标怎么样？", // PENDING-RESX (innocent survey face — never reveal it's a gate)
  "Welcome_GateTitle": "开始前，一个小问题", // PENDING-RESX
  "Welcome_GateYes": "丑，忍很久了", // PENDING-RESX
  "Welcome_GateYesNote": "放心：区分快捷方式的功能会留着，只是不准它丑。", // PENDING-RESX
  "Welcome_In": "进来吧，这里全是同类。", // PENDING-RESX
  "Welcome_NoPaste": "想得美，粘贴不算。", // PENDING-RESX
  "Welcome_Promise": "把参差不齐的桌面，收拾成一件作品。", // PENDING-RESX
  "Welcome_RoastBody": "这不是我们的目标用户，请赶紧卸载。", // PENDING-RESX (owner verbatim)
  "Welcome_RoastRethink": "等等，我再想想", // PENDING-RESX
  "Welcome_RoastTitle": "那我们直说了", // PENDING-RESX
  "Welcome_RoastUninstall": "好，我去卸载", // PENDING-RESX
  "Welcome_Start": "开始", // PENDING-RESX
  "Welcome_TypeHere": "在这里抄", // PENDING-RESX
  "Zone_Accent": "强调色",
  "Zone_ApplyAll": "应用到全部分区",
  "Zone_CopySuffix": "副本",
  "Zone_DefaultTitle": "新分区",
  "Zone_DeletedToast": "已删除 {0}",
  "Zone_EditingHeader": "正在编辑：{0}",
  "Zone_EditingNone": "未选择分区",
  "Zone_EmojiCustom": "输入任意表情",
  "Zone_EmojiNone": "无",
  "Zone_EmojiOsHint": "按 Win + 句号 打开系统表情面板",
  "Zone_EmojiTabFaces": "表情",
  "Zone_EmojiTabObjects": "物品",
  "Zone_Material": "材质",
  "Zone_Multiple": "多个分区",
  "Zone_SelectHint": "选择一个分区来调整它的样式",
  "Zone_Shadow": "投影",
  "Zone_TitleApps": "常用软件",
  "Zone_TitleArchive": "归档",
  "Zone_TitleDoing": "正在进行",
  "Zone_TitleInbox": "收件箱",
  "Zone_TitleStyle": "标题样式",
  "Zone_TitleWork": "工作文件",
  "Zone_ToneAuto": "自动",
  "Zone_ToneDark": "深",
  "Zone_ToneLight": "浅",
  "Zoom_FitAll_Tip": "全览整个桌面",
  "Zoom_FitHeight_Tip": "满高显示 · 靠左",
  "Zoom_FitWidth_Tip": "满宽显示",
  "Zoom_In_Tip": "放大",
  "Zoom_Out_Tip": "缩小",
}
