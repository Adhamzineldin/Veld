const REPO = 'https://github.com/Adhamzineldin/Veld';

/** Tells the reader that these docs describe Veld v1 and points to the v2 documentation. */
export default function V1Banner() {
  return (
    <div
      role="note"
      style={{
        background: '#3d2e00',
        color: '#ffd666',
        borderBottom: '1px solid #7a5c00',
        padding: '12px 24px',
        textAlign: 'center',
        fontSize: 14,
        lineHeight: 1.5,
      }}
    >
      <strong>These docs describe Veld v1.</strong> Veld 2.0 is a Rust rewrite with a new language.
      Read the <a href={`${REPO}#readme`} style={{ color: 'inherit', textDecoration: 'underline' }}>2.0 README</a>{' '}
      and the <a href={`${REPO}/tree/master/spec`} style={{ color: 'inherit', textDecoration: 'underline' }}>specs</a>.
      Convert a v1 project with <code>veld migrate</code>.
    </div>
  );
}
