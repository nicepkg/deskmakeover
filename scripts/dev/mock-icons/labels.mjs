// Invented, messy CN+EN desktop labels — zero real trademarks (spec 06 §5).

import { pick } from './prng.mjs'

const FAKE_EN = ['PhotonChat', 'DevForge', 'MeteorPlay', 'NovaEdit', 'PixelForge', 'AuroraDB', 'VoltMail', 'QuellNote', 'DriftSync', 'EmberCast', 'HollowIDE', 'LumenCAD', 'GristCalc', 'ZephyrVPN', 'TidalDraw', 'FluxBoard', 'CinderTerm', 'OrbitPay']
const FAKE_ZH = ['星穹笔记', '云图相册', '墨刻文档', '潮汐音乐', '微光邮箱', '折光剪辑', '蜂巢云盘', '拾光日记', '磐石数据库', '流沙同步', '暗河终端', '朝霞画板']
const DESK_ZH = ['工作文档', '季度报表', '原型稿_v3', '会议纪要', '项目计划', '本地相册', '读书清单', '报销单', '某某启动器', '待办清单', '装机必备', '临时文件', '家庭账本']
const FOLDER_LB = ['素材库', '下载', '项目归档', '2024备份', '截图', 'Documents', 'Projects', '工作', '家庭照片', '设计稿', 'node_modules', '临时']
const DOC_LB = ['合同_最终版', '预算表_2024', '周报', '需求文档', '说明书', 'README', '简历_2024']
const URL_LB = ['内网门户', '打卡系统', '工单平台', 'DevPortal', 'StatusPage', '知识库', '监控大盘']

export function labelFor(rng, cat, kind) {
  if (kind === 'folder') return pick(rng, FOLDER_LB)
  if (kind === 'file') return pick(rng, DOC_LB) + pick(rng, ['.pdf', '.docx', '.xlsx', '.txt', ''])
  if (kind === 'url') return pick(rng, URL_LB)
  if (kind === 'bin') return pick(rng, ['回收站', '废纸篓'])
  const base = pick(rng, [...FAKE_EN, ...FAKE_ZH, ...DESK_ZH])
  return rng() < 0.22 ? base + pick(rng, [' (1)', '_副本', ' - 快捷方式', '_v2']) : base
}
