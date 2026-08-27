import { Code2, LoaderCircle } from "lucide-react";

type Props = {
  title?: string;
  subtitle?: string;
  fullScreen?: boolean;
};

export function LoadingScreen({
  title = "Loading",
  subtitle = "Just a moment…",
  fullScreen = false,
}: Props) {
  return (
    <div className={`loading-screen ${fullScreen ? "loading-screen-full" : ""}`} role="status" aria-live="polite">
      <div className="loading-screen-inner">
        <div className="loading-screen-mark">
          <Code2 size={20} />
        </div>
        <div className="loading-screen-copy">
          <strong>{title}</strong>
          <span>{subtitle}</span>
        </div>
        <div className="loading-bar" aria-hidden>
          <div className="loading-bar-fill" />
        </div>
        <LoaderCircle size={14} className="loading-screen-spin spin" aria-hidden />
      </div>
    </div>
  );
}

export function ProjectCardSkeleton() {
  return (
    <article className="project-card skeleton-card" aria-hidden>
      <div className="skeleton-card-head">
        <div className="skeleton-block skeleton-icon" />
        <div className="skeleton-lines">
          <div className="skeleton-block skeleton-title" />
          <div className="skeleton-block skeleton-sub" />
        </div>
      </div>
      <div className="skeleton-block skeleton-meta" />
      <div className="skeleton-block skeleton-meta" />
      <div className="skeleton-actions">
        <div className="skeleton-block skeleton-btn wide" />
        <div className="skeleton-block skeleton-btn" />
      </div>
    </article>
  );
}

export function ProjectGridSkeleton({ count = 3 }: { count?: number }) {
  return (
    <div className="project-grid" aria-busy="true" aria-label="Loading projects">
      {Array.from({ length: count }, (_, index) => (
        <ProjectCardSkeleton key={index} />
      ))}
    </div>
  );
}
