import { Wordmark } from "../components/Wordmark";

export function LoadingScreen() {
  return (
    <div className="bg-bg flex h-full items-center justify-center">
      <div className="animate-fade-in text-center">
        <Wordmark size={48} className="text-text" />
        <p className="text-text-2 mt-4 text-sm">Loading...</p>
      </div>
    </div>
  );
}
