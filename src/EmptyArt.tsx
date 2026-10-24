/** Two speech bubbles with transcript lines: the empty-state picture for Home and folders. */
export function EmptyArt() {
  return (
    <svg className="empty-art" width="168" height="112" viewBox="0 0 168 112" aria-hidden focusable="false">
      <rect x="8" y="10" width="104" height="58" rx="18" fill="var(--raised)" stroke="var(--hairline-strong)" />
      <path d="M30 68 l-6 16 l20 -16z" fill="var(--raised)" stroke="var(--hairline-strong)" strokeLinejoin="round" />
      <rect x="26" y="28" width="56" height="7" rx="3.5" fill="var(--hairline-strong)" />
      <rect x="26" y="43" width="72" height="7" rx="3.5" fill="var(--hairline)" />
      <rect x="64" y="46" width="96" height="52" rx="18" fill="var(--accent-soft)" stroke="var(--accent)" />
      <path d="M140 98 l6 12 l-18 -12z" fill="var(--accent-soft)" stroke="var(--accent)" strokeLinejoin="round" />
      <circle cx="94" cy="72" r="5" fill="var(--accent)" />
      <circle cx="112" cy="72" r="5" fill="var(--accent)" />
      <circle cx="130" cy="72" r="5" fill="var(--accent)" />
    </svg>
  );
}
