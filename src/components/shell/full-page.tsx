import type { ReactNode } from 'react'

// The ONE full-page layout (owner 2026-07-13): every full-page module (设置,
// 清爽, future pages) shares this shell so the title sits at the identical
// position everywhere — same max width, same padding, same header rhythm.
// Canvas+inspector modules use ModuleLayout instead; this is its page sibling.

export function FullPage({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[1080px] px-10 py-8">
        <header className="mb-6">
          <h1 className="text-display font-medium text-t1">{title}</h1>
        </header>
        {children}
      </div>
    </div>
  )
}
