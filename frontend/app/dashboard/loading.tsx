import LoadingSpinner from '../../components/LoadingSpinner';

export default function Loading() {
  return (
    <div className="flex h-[70vh] items-center justify-center">
      <LoadingSpinner label="Booting War Room..." />
    </div>
  );
}

