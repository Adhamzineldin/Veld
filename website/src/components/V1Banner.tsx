const REPO = 'https://github.com/Adhamzineldin/Veld';

/** A small link for readers who are still on Veld 1. */
export default function V1Banner() {
  return (
    <div
      style={{
        padding: '8px 24px',
        textAlign: 'right',
        fontSize: 13,
        color: 'var(--fg2)',
      }}
    >
      Looking for v1?{' '}
      <a href={`${REPO}/tree/v1-final`} style={{ color: 'inherit', textDecoration: 'underline' }}>
        See tag v1-final
      </a>
    </div>
  );
}
