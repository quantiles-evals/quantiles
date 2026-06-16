import { emit, entrypoint, step, workflow } from "../src/index.js";

type Label = "billing" | "bug" | "how_to";
type PromptName = "A" | "B";

interface EvalInput {
  prompt?: PromptName;
}

interface EvalCase {
  id: string;
  ticket: string;
  expected: Label;
}

interface CaseResult {
  id: string;
  expected: Label;
  prediction: Label;
  correct: boolean;
  tokensUsed: number;
}

interface EvalSummary {
  promptName: PromptName;
  accuracy: number;
  correct: number;
  total: number;
  tokensUsed: number;
  costUsd: number;
  results: CaseResult[];
}

const prompts: Record<PromptName, string> = {
  A: "Classify this support ticket as billing, bug, or how_to. Return only the label.",
  B: "You are a support triage assistant. Classify the ticket into exactly one label: billing, bug, or how_to. Return only the label.",
};

const evalCases: EvalCase[] = [
  {
    id: "double-charge",
    ticket: "I was charged twice for my subscription renewal.",
    expected: "billing",
  },
  {
    id: "csv-crash",
    ticket: "The app crashes every time I upload a CSV file.",
    expected: "bug",
  },
  {
    id: "export-invoices",
    ticket: "How do I export all invoices for last quarter?",
    expected: "how_to",
  },
  {
    id: "password-reset",
    ticket: "Where can I reset my password if I forgot it?",
    expected: "how_to",
  },
  {
    id: "refund-request",
    ticket: "I canceled yesterday and need a refund for the annual plan.",
    expected: "billing",
  },
  {
    id: "blank-dashboard",
    ticket: "My dashboard is blank after the latest update.",
    expected: "bug",
  },
  {
    id: "change-card",
    ticket: "How can I change the credit card used for billing?",
    expected: "billing",
  },
  {
    id: "invite-teammate",
    ticket: "Can you show me how to invite another teammate?",
    expected: "how_to",
  },
  {
    id: "api-timeout",
    ticket: "The API times out whenever I create a report.",
    expected: "bug",
  },
  {
    id: "download-receipt",
    ticket: "I need a receipt for my most recent payment.",
    expected: "billing",
  },
  {
    id: "setup-webhook",
    ticket: "What are the steps to configure a webhook endpoint?",
    expected: "how_to",
  },
  {
    id: "mobile-freeze",
    ticket: "The mobile app freezes on the login screen.",
    expected: "bug",
  },
];

const promptEval = workflow<EvalInput, EvalSummary>(
  "support-triage",
  async (input = {}) => {
    const promptName = input.prompt === "B" ? "B" : "A";
    const prompt = prompts[promptName];

    const results: CaseResult[] = [];
    let correct = 0;
    let tokensUsed = 0;

    for (const evalCase of evalCases) {
      // TODO: change to `task` instead of `step`
      const result = await step(
        `case:${evalCase.id}`,
        {
          promptName,
          prompt,
          ticket: evalCase.ticket,
        },
        async () => {
          const prediction = fakeModel(promptName, evalCase.ticket);
          const caseTokens = estimateTokens(prompt, evalCase.ticket);

          return {
            id: evalCase.id,
            expected: evalCase.expected,
            prediction,
            correct: prediction === evalCase.expected,
            tokensUsed: caseTokens,
          };
        },
      );

      results.push(result);
      if (result.correct) {
        correct += 1;
      }
      tokensUsed += result.tokensUsed;
    }

    const total = evalCases.length;
    const accuracy = correct / total;
    const costUsd = tokensUsed * 0.000001;

    await emit("accuracy", accuracy);
    await emit("correct_count", correct);
    await emit("total_count", total);
    await emit("tokens_used", tokensUsed);
    await emit("cost_usd", costUsd);

    console.log(
      `Prompt ${promptName}: ${correct}/${total} correct (${Math.round(
        accuracy * 100,
      )}%)`,
    );
    console.log(`Tokens used: ${tokensUsed}`);

    return {
      promptName,
      accuracy,
      correct,
      total,
      tokensUsed,
      costUsd,
      results,
    };
  },
);

function fakeModel(promptName: PromptName, ticket: string): Label {
  if (promptName === "B") {
    return classifyWithRules(ticket);
  }

  const lower = ticket.toLowerCase();
  if (
    lower.includes("how") ||
    lower.includes("steps") ||
    lower.includes("show me")
  ) {
    return "how_to";
  }
  if (
    lower.includes("crashes") ||
    lower.includes("blank") ||
    lower.includes("freezes")
  ) {
    return "bug";
  }
  return "billing";
}

function classifyWithRules(ticket: string): Label {
  const lower = ticket.toLowerCase();
  if (lower.includes("export all invoices")) {
    return "how_to";
  }
  if (
    lower.includes("crash") ||
    lower.includes("blank") ||
    lower.includes("timeout") ||
    lower.includes("times out") ||
    lower.includes("freeze")
  ) {
    return "bug";
  }
  if (
    lower.includes("charged") ||
    lower.includes("refund") ||
    lower.includes("credit card") ||
    lower.includes("payment") ||
    lower.includes("invoice") ||
    lower.includes("receipt")
  ) {
    return "billing";
  }
  return "how_to";
}

function estimateTokens(prompt: string, ticket: string): number {
  return Math.ceil(`${prompt} ${ticket}`.split(/\s+/).length * 1.3);
}

entrypoint(promptEval);
