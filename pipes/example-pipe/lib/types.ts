import type { Settings as SkypromptAppSettings } from "@skyprompt/js";

export interface WorkLog {
  title: string;
  description: string;
  tags: string[];
  startTime: string;
  endTime: string;
}

export interface Contact {
  name: string;
  company?: string;
  lastInteraction: string;
  sentiment: number;
  topics: string[];
  nextSteps: string[];
}

export interface Intelligence {
  contacts: Contact[];
  insights: {
    followUps: string[];
    opportunities: string[];
  };
}

export interface Settings {
  exampleSetting?: string;
  prompt: string;
  vaultPath: string;
  logTimeWindow: number;
  logPageSize: number;
  logModel: string;
  analysisModel: string;
  analysisTimeWindow: number;
  deduplicationEnabled: boolean;
  skypromptAppSettings: SkypromptAppSettings;
}
