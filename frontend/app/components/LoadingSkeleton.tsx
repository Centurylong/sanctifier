interface LoadingSkeletonProps {
  className?: string;
  count?: number;
}

export function LoadingSkeleton({ className = "", count = 1 }: LoadingSkeletonProps) {
  return (
    <>
      {Array.from({ length: count }).map((_, i) => (
        <div
          key={i}
          className={`skeleton rounded-lg ${className}`}
          role="status"
          aria-label="Loading"
        >
          <span className="sr-only">Loading...</span>
        </div>
      ))}
    </>
  );
}

export function DashboardSkeleton() {
  return (
    <div className="space-y-8" role="status" aria-label="Loading dashboard">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <LoadingSkeleton className="h-48" />
        <LoadingSkeleton className="h-48" />
      </div>
      <LoadingSkeleton className="h-32" />
      <div className="space-y-4">
        <LoadingSkeleton className="h-24" count={3} />
      </div>
      <span className="sr-only">Loading dashboard data...</span>
    </div>
  );
}
