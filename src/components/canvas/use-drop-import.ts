import * as React from 'react'
import { useT } from '@/lib/i18n'
import { useWallpaper } from '@/stores/wallpaper'
import { useToasts } from '@/stores/toasts'

// OS file drag-drop → import a wallpaper source. Split from wallpaper-mirror.tsx for
// the ≤500-line law; behaviour verbatim. OS file drags never fire pointer events, so
// this never clashes with the canvas zone-drag gestures. Returns `dropActive` (the
// coral drop-ring overlay) + the drag handlers to spread onto the canvas host.

export function useDropImport(): {
  dropActive: boolean
  dropHandlers: {
    onDragOver: (e: React.DragEvent) => void
    onDragLeave: () => void
    onDrop: (e: React.DragEvent) => void
  }
} {
  const t = useT()
  const [dropActive, setDropActive] = React.useState(false)

  const hasImageFile = (e: React.DragEvent) => [...e.dataTransfer.items].some((i) => i.kind === 'file')
  const onDragOver = (e: React.DragEvent) => {
    if (!hasImageFile(e)) return
    e.preventDefault()
    setDropActive(true)
  }
  const onDragLeave = () => setDropActive(false)
  const onDrop = (e: React.DragEvent) => {
    e.preventDefault()
    setDropActive(false)
    const file = e.dataTransfer.files[0]
    if (!file) return
    if (!file.type.startsWith('image/')) {
      useToasts.getState().show(t('Paper_DropReject'), 'warn')
      return
    }
    void useWallpaper.getState().importSource(file).then((ok) => {
      if (!ok) useToasts.getState().show(t('Toast_ImportFailed'), 'warn')
    })
  }

  return { dropActive, dropHandlers: { onDragOver, onDragLeave, onDrop } }
}
