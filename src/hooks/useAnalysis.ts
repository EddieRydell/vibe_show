import { useState, useCallback } from "react";
import type {
  AudioAnalysis,
  PythonEnvStatus,
  AnalysisFeatures,
} from "../types";
import { cmd } from "../commands";

export function useAnalysis() {
  const [analysis, setAnalysis] = useState<AudioAnalysis | null>(null);
  const [pythonStatus, setPythonStatus] = useState<PythonEnvStatus | null>(
    null,
  );
  const [analyzing, setAnalyzing] = useState(false);
  const [settingUp, setSettingUp] = useState(false);

  const checkPython = useCallback(async () => {
    const status = await cmd.getPythonStatus();
    setPythonStatus(status);
    return status;
  }, []);

  const setupPython = useCallback(async () => {
    setSettingUp(true);
    try {
      await cmd.setupPythonEnv();
      await checkPython();
    } finally {
      setSettingUp(false);
    }
  }, [checkPython]);

  const refreshAnalysis = useCallback(async () => {
    const cached = await cmd.getAnalysis();
    setAnalysis(cached);
    return cached;
  }, []);

  const runAnalysis = useCallback(
    async (features?: AnalysisFeatures) => {
      setAnalyzing(true);
      try {
        const result = await cmd.analyzeAudio(features);
        setAnalysis(result);
        return result;
      } finally {
        setAnalyzing(false);
      }
    },
    [],
  );

  return {
    analysis,
    pythonStatus,
    analyzing,
    settingUp,
    checkPython,
    setupPython,
    runAnalysis,
    refreshAnalysis,
  };
}
