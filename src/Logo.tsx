/** The Parley mark: a speech bubble whose tail is the stem of a P. Uses the current accent. */
export function Logo({ size = 28 }: { size?: number }) {
  return (
    <svg className="logo" width={size} height={size * (620 / 560)} viewBox="260 220 560 620" aria-hidden focusable="false">
      <path
        fill="var(--accent)"
        d="M356 290h188c112 0 202 90 202 200s-90 200-202 200H440v62c0 30-24 54-54 54h-2c-30 0-54-24-54-54V316c0-14 12-26 26-26z"
      />
      <circle cx="454" cy="490" r="30" fill="var(--paper)" />
      <circle cx="546" cy="490" r="30" fill="var(--paper)" />
      <circle cx="638" cy="490" r="30" fill="var(--paper)" />
    </svg>
  );
}
