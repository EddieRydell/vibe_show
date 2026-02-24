import { Wordmark } from "../components/Wordmark";

export function LoadingScreen() {
  return (
    <div className="flex h-full items-center justify-center bg-bg/80 backdrop-blur-sm animate-fade-in-delayed">
      <div className="text-center">
        <Wordmark size={48} className="text-text" />
        <p className="text-text-2 mt-4 text-sm">Loading...</p>
      </div>
    </div>
  );
}
