const svg = (body: string, fill = "none"): string =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="${fill}" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`

export const icons = {
  compose: svg('<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"/>'),
  search: svg('<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>'),
  sidebar: svg('<rect width="18" height="18" x="3" y="3" rx="3"/><path d="M9 3v18"/>'),
  panelRight: svg('<rect width="18" height="18" x="3" y="3" rx="3"/><path d="M15 3v18"/>'),
  arrowLeft: svg('<path d="m15 18-6-6 6-6"/>'),
  arrowRight: svg('<path d="m9 18 6-6-6-6"/>'),
  folder: svg('<path d="M3 6h6l2 2h10v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"/><path d="M3 10h18"/>'),
  settings: svg(
    '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.09A1.7 1.7 0 0 0 9 19.37a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.63 15 1.7 1.7 0 0 0 3.09 14H3v-4h.09A1.7 1.7 0 0 0 4.63 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.63 1.7 1.7 0 0 0 10 3.09V3h4v.09A1.7 1.7 0 0 0 15 4.63a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.37 9 1.7 1.7 0 0 0 20.91 10H21v4h-.09A1.7 1.7 0 0 0 19.4 15Z"/>',
  ),
  gitBranch: svg(
    '<line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>',
  ),
  laptop: svg('<rect width="18" height="12" x="3" y="4" rx="2"/><path d="M2 20h20"/>'),
  lockOpen: svg(
    '<rect width="18" height="11" x="3" y="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/>',
  ),
  lock: svg(
    '<rect width="18" height="11" x="3" y="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  ),
  list: svg('<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>'),
  zap: svg('<path d="M13 2 3 14h9l-1 8 10-12h-9Z"/>'),
  pencil: svg('<path d="m15 5 4 4M3 21l4-1 13-13a2.83 2.83 0 0 0-4-4L3 16Z"/>'),
  chevronDown: svg('<path d="m6 9 6 6 6-6"/>'),
  chevronRight: svg('<path d="m9 6 6 6-6 6"/>'),
  listFilter: svg('<path d="M3 6h18M7 12h10M10 18h4"/>'),
  sparkle: svg('<path d="M12 3v18M4.2 7.5l15.6 9M19.8 7.5l-15.6 9"/>'),
  wrench: svg(
    '<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.8-3.8a6 6 0 0 1-8 8l-6.9 6.9a2.1 2.1 0 0 1-3-3l6.9-6.9a6 6 0 0 1 8-8Z"/>',
  ),
  send: svg('<path d="m5 12 7-7 7 7M12 19V5"/>'),
  copy: svg(
    '<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>',
  ),
  check: svg('<path d="M20 6 9 17l-5-5"/>'),
  retry: svg('<path d="M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5"/>'),
  thumbsUp: svg(
    '<path d="M7 10v12M15 5.9 14 10h5.8a2 2 0 0 1 1.9 2.6l-2.3 8A2 2 0 0 1 17.5 22H4a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h2.8a2 2 0 0 0 1.8-1.1L12 2a3.1 3.1 0 0 1 3 3.9Z"/>',
  ),
  thumbsDown: svg(
    '<path d="M17 14V2M9 18.1 10 14H4.2a2 2 0 0 1-1.9-2.6l2.3-8A2 2 0 0 1 6.5 2H20a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2.8a2 2 0 0 0-1.8 1.1L12 22a3.1 3.1 0 0 1-3-3.9Z"/>',
  ),
  share: svg('<path d="M12 2v13m4-9-4-4-4 4M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8"/>'),
  more: svg(
    '<circle cx="5" cy="12" r="1" fill="#000"/><circle cx="12" cy="12" r="1" fill="#000"/><circle cx="19" cy="12" r="1" fill="#000"/>',
  ),
} as const

export type IconName = keyof typeof icons
