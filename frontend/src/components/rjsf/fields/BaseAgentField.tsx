import { FieldProps, FieldPathId, RJSFSchema } from '@rjsf/utils';
import { useMemo, useState, useCallback, useEffect } from 'react';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

const AGENT_TYPES = [
  { value: 'CLAUDE_CODE', label: 'Claude Code' },
  { value: 'AMP', label: 'AMP' },
  { value: 'GEMINI', label: 'Gemini' },
  { value: 'CODEX', label: 'Codex' },
  { value: 'OPENCODE', label: 'OpenCode' },
  { value: 'CURSOR_AGENT', label: 'Cursor Agent' },
  { value: 'QWEN_CODE', label: 'Qwen Code' },
  { value: 'COPILOT', label: 'Copilot' },
  { value: 'DROID', label: 'Droid' },
] as const;

type BaseAgentConfig = Record<string, unknown>;

export function BaseAgentField({
  formData,
  onChange,
  disabled,
  readonly,
  schema,
  registry,
}: FieldProps<BaseAgentConfig>) {
  // Get the Fields from registry
  const { fields } = registry;

  // State for the selected type - this is the source of truth for the dropdown
  // We detect the initial type from formData keys
  const initialType = useMemo(() => {
    if (!formData) return 'CLAUDE_CODE';
    for (const agent of AGENT_TYPES) {
      if (formData[agent.value] !== undefined) {
        return agent.value;
      }
    }
    return 'CLAUDE_CODE';
  }, [formData]);

  const [selectedType, setSelectedType] = useState(initialType);

  // Update selectedType when formData changes (e.g., on initial load)
  const detectedType = useMemo(() => {
    if (!formData) return null;
    for (const agent of AGENT_TYPES) {
      if (formData[agent.value] !== undefined) {
        return agent.value;
      }
    }
    return null;
  }, [formData]);

  // Sync local state when formData indicates a different type (only on initial load)
  useEffect(() => {
    if (detectedType && detectedType !== selectedType) {
      setSelectedType(detectedType);
    }
  }, [detectedType]);

  // Get the schema for the selected agent type
  const selectedAgentSchema = useMemo(() => {
    const properties = schema.properties as Record<string, RJSFSchema> | undefined;
    if (!properties) return null;
    return properties[selectedType] ?? null;
  }, [schema, selectedType]);

  // Handler for type selection change
  const handleTypeChange = useCallback((newType: string) => {
    setSelectedType(newType as typeof AGENT_TYPES[number]['value']);
  }, []);

  // Handler for nested field changes - updates formData with new values
  const handleFieldChange = useCallback((key: string, newValue: unknown) => {
    // Build formData with only the selected agent's config
    // This ensures we don't save configs for unselected agents
    const selectedConfig = {
      ...((formData?.[selectedType] as Record<string, unknown>) || {}),
      [key]: newValue,
    };

    // Create clean formData with only the selected agent
    const newFormData: BaseAgentConfig = {
      [selectedType]: selectedConfig,
    };

    onChange(newFormData, []);
  }, [formData, selectedType, onChange]);

  const isDisabled = disabled || readonly;

  // For the selected agent, get its nested properties schema
  const nestedSchema = useMemo(() => {
    if (!selectedAgentSchema?.properties) return null;
    return {
      type: 'object' as const,
      title: `${AGENT_TYPES.find((a) => a.value === selectedType)?.label} Settings`,
      properties: selectedAgentSchema.properties,
      required: selectedAgentSchema.required,
    };
  }, [selectedAgentSchema, selectedType]);

  // Get the config for the selected agent - only use selectedType from formData
  const agentConfig = formData?.[selectedType] as Record<string, unknown> | undefined;

  return (
    <div className="space-y-4">
      {/* Type selector */}
      <div className="space-y-2">
        <Label htmlFor="base-agent-type">Base Agent Type</Label>
        <Select
          value={selectedType}
          onValueChange={handleTypeChange}
          disabled={isDisabled}
        >
          <SelectTrigger id="base-agent-type" className="w-full">
            <SelectValue placeholder="Select agent type" />
          </SelectTrigger>
          <SelectContent>
            {AGENT_TYPES.map((agent) => (
              <SelectItem key={agent.value} value={agent.value}>
                {agent.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <p className="text-sm text-muted-foreground">
          Select the base agent type to customize
        </p>
      </div>

      {/* Agent-specific config - only show for selected type */}
      {nestedSchema && (
        <div className="border rounded-md p-4 space-y-4">
          <Label className="text-base font-medium">
            {AGENT_TYPES.find((a) => a.value === selectedType)?.label} Settings
          </Label>
          {/* Render the nested properties manually */}
          {Object.entries(nestedSchema.properties || {}).map(([key, propSchema]) => {
            const Field = fields[key as keyof typeof fields];
            if (!Field) return null;

            // Get the value from formData for this specific key
            const value = agentConfig?.[key];

            // Get the schema for this field
            const fieldSchema = propSchema as RJSFSchema;

            return (
              <Field
                key={key}
                name={key}
                schema={fieldSchema}
                uiSchema={{}}
                formData={value as BaseAgentConfig | undefined}
                onChange={(newValue: unknown) => handleFieldChange(key, newValue)}
                onBlur={() => {}}
                onFocus={() => {}}
                disabled={isDisabled}
                readonly={readonly}
                registry={registry}
                fieldPathId={`root.${key}` as unknown as FieldPathId}
              />
            ) as React.ReactElement;
          })}
        </div>
      )}
    </div>
  );
}
