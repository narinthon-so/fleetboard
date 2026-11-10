export function BrandLogo({ className = '' }: { className?: string }) {
  return (
    <span role="img" aria-label="fleetboard" className={`inline-block shrink-0 ${className}`}>
      <img
        src="/brand/fleetboard-horizontal-color.svg"
        alt=""
        width="1154.7"
        height="256"
        className="brand-logo-light w-full h-auto"
      />
      <img
        src="/brand/fleetboard-horizontal-white.svg"
        alt=""
        width="1154.7"
        height="256"
        className="brand-logo-dark w-full h-auto"
      />
    </span>
  );
}
