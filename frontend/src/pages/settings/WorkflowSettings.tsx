import { useState, useEffect } from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Checkbox } from '@/components/ui/checkbox';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Loader2, Save } from 'lucide-react';
import { useProjectWorkflowConfig, useUpdateProjectWorkflowConfig } from '@/hooks/useProjectWorkflowConfig';
import type { UpdateWorkflowConfigRequest, WorkflowConfigResponse } from 'shared/types';

interface WorkflowSettingsProps {
  projectId: string;
}

export function WorkflowSettings({ projectId }: WorkflowSettingsProps) {
  const [localConfig, setLocalConfig] = useState<WorkflowConfigResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Fetch current workflow config
  const {
    data: config,
    isLoading: loadingConfig,
    error: fetchError,
  } = useProjectWorkflowConfig(projectId);

  // Mutation for updating config
  const updateConfig = useUpdateProjectWorkflowConfig(projectId);

  // Sync local state with fetched config
  useEffect(() => {
    if (config) {
      setLocalConfig(config);
    }
  }, [config]);

  const handleToggleChange = (field: keyof WorkflowConfigResponse, checked: boolean) => {
    if (!localConfig) return;
    setLocalConfig((prev) => {
      if (!prev) return prev;
      return { ...prev, [field]: checked };
    });
  };

  const handleNumberChange = (field: keyof WorkflowConfigResponse, value: string) => {
    if (!localConfig) return;
    const numValue = parseInt(value, 10);
    if (isNaN(numValue) || numValue < 1) return; // Validate minimum value
    setLocalConfig((prev) => {
      if (!prev) return prev;
      return { ...prev, [field]: numValue };
    });
  };

  const handleTemplateChange = (value: string | null) => {
    if (!localConfig) return;
    setLocalConfig((prev) => {
      if (!prev) return prev;
      return { ...prev, ai_review_prompt_template: value };
    });
  };

  const handleSave = async () => {
    if (!localConfig) return;

    setError(null);
    setSuccess(false);

    try {
      const updateData: UpdateWorkflowConfigRequest = {
        enable_human_review: localConfig.enable_human_review,
        max_ai_review_iterations: localConfig.max_ai_review_iterations,
        testing_requires_manual_exit: localConfig.testing_requires_manual_exit,
        auto_start_ai_review: localConfig.auto_start_ai_review,
        ai_review_prompt_template: localConfig.ai_review_prompt_template,
      };

      await updateConfig.mutateAsync(updateData);
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save workflow settings');
    }
  };

  const handleReset = () => {
    if (config) {
      setLocalConfig(config);
    }
  };

  const hasChanges = localConfig && config && (
    localConfig.enable_human_review !== config.enable_human_review ||
    localConfig.max_ai_review_iterations !== config.max_ai_review_iterations ||
    localConfig.testing_requires_manual_exit !== config.testing_requires_manual_exit ||
    localConfig.auto_start_ai_review !== config.auto_start_ai_review ||
    localConfig.ai_review_prompt_template !== config.ai_review_prompt_template
  );

  if (loadingConfig) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Workflow Settings</CardTitle>
          <CardDescription>Configure how tasks move through your workflow</CardDescription>
        </CardHeader>
        <CardContent className="flex items-center justify-center py-8">
          <Loader2 className="h-6 w-6 animate-spin" />
          <span className="ml-2 text-muted-foreground">Loading workflow settings...</span>
        </CardContent>
      </Card>
    );
  }

  if (fetchError) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Workflow Settings</CardTitle>
          <CardDescription>Configure how tasks move through your workflow</CardDescription>
        </CardHeader>
        <CardContent>
          <Alert variant="destructive">
            <AlertDescription>
              {fetchError instanceof Error
                ? fetchError.message
                : 'Failed to load workflow settings'}
            </AlertDescription>
          </Alert>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Workflow Settings</CardTitle>
        <CardDescription>
          Configure how tasks move through your workflow. These settings affect status transitions
          and review processes.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {success && (
          <Alert variant="success">
            <AlertDescription className="font-medium">
              Workflow settings saved successfully
            </AlertDescription>
          </Alert>
        )}

        {localConfig && (
          <>
            {/* Human Review Toggle */}
            <div className="space-y-3 pt-2">
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="enable-human-review" className="text-base">
                    Human Review
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    Allow tasks to be reviewed by a human before completion
                  </p>
                </div>
                <Checkbox
                  id="enable-human-review"
                  checked={localConfig.enable_human_review}
                  onCheckedChange={(checked) =>
                    handleToggleChange('enable_human_review', checked === true)
                  }
                />
              </div>
              {localConfig.enable_human_review && (
                <div className="ml-6 p-3 bg-muted rounded-md">
                  <p className="text-sm">
                    When enabled, tasks can transition from <code>InReview</code> to{' '}
                    <code>HumanReview</code> for manual review before completion.
                  </p>
                </div>
              )}
            </div>

            <div className="border-t" />

            {/* Testing Phase Toggle */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="testing-manual-exit" className="text-base">
                    Require Manual Exit from Testing
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    Tasks must go through Testing phase before AI Review (recommended)
                  </p>
                </div>
                <Checkbox
                  id="testing-manual-exit"
                  checked={localConfig.testing_requires_manual_exit}
                  onCheckedChange={(checked) =>
                    handleToggleChange('testing_requires_manual_exit', checked === true)
                  }
                />
              </div>
              {!localConfig.testing_requires_manual_exit && (
                <div className="ml-6 p-3 bg-amber-50 dark:bg-amber-950 rounded-md border border-amber-200 dark:border-amber-800">
                  <p className="text-sm text-amber-800 dark:text-amber-200">
                    When disabled, tasks can bypass the Testing phase and go directly from{' '}
                    <code>InProgress</code> to <code>InReview</code>.
                  </p>
                </div>
              )}
            </div>

            <div className="border-t" />

            {/* AI Review Settings */}
            <div className="space-y-4">
              <Label className="text-base">AI Review Configuration</Label>

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-2">
                  <Label htmlFor="max-ai-iterations">Max AI Review Iterations</Label>
                  <Input
                    id="max-ai-iterations"
                    type="number"
                    min="1"
                    max="20"
                    value={localConfig.max_ai_review_iterations}
                    onChange={(e) =>
                      handleNumberChange('max_ai_review_iterations', e.target.value)
                    }
                  />
                  <p className="text-sm text-muted-foreground">
                    Maximum review cycles before requiring human intervention (minimum: 1)
                  </p>
                </div>

                <div className="flex items-center pt-6">
                  <Checkbox
                    id="auto-start-ai-review"
                    checked={localConfig.auto_start_ai_review}
                    onCheckedChange={(checked) =>
                      handleToggleChange('auto_start_ai_review', checked === true)
                    }
                  />
                  <Label htmlFor="auto-start-ai-review" className="ml-2 font-normal">
                    Auto-start AI Review when entering Testing phase
                  </Label>
                </div>
              </div>
            </div>

            <div className="border-t" />

            {/* AI Review Prompt Template */}
            <div className="space-y-2">
              <Label htmlFor="ai-review-prompt" className="text-base">
                AI Review Prompt Template
              </Label>
              <textarea
                id="ai-review-prompt"
                value={localConfig.ai_review_prompt_template ?? ''}
                onChange={(e) => handleTemplateChange(e.target.value || null)}
                placeholder="Enter custom instructions for AI review..."
                rows={4}
                className="w-full px-3 py-2 border border-input bg-background text-foreground rounded-md focus:outline-none focus:ring-2 focus:ring-ring font-mono text-sm"
              />
              <p className="text-sm text-muted-foreground">
                Custom prompt template for AI code review. Leave empty to use system default.
              </p>
            </div>

            {/* Save Buttons */}
            <div className="flex items-center justify-between pt-4 border-t">
              <Button variant="outline" onClick={handleReset} disabled={!hasChanges}>
                Discard Changes
              </Button>
              <Button onClick={handleSave} disabled={!hasChanges || updateConfig.isPending}>
                {updateConfig.isPending ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    Saving...
                  </>
                ) : (
                  <>
                    <Save className="h-4 w-4 mr-2" />
                    Save Settings
                  </>
                )}
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
