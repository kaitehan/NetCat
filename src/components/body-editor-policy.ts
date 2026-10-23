export type BodyLanguage = 'json' | 'xml' | 'html' | 'javascript' | 'css' | 'plaintext'

export const LARGE_BODY_THRESHOLD = 1024 * 1024

export interface BodyEditorPolicy {
  value: string
  language: BodyLanguage
  large: boolean
  diagnostics: boolean
}

export function detectBodyLanguage(
  contentType: string | null | undefined,
  value: string
): BodyLanguage {
  const normalized = contentType?.split(';', 1)[0]?.trim().toLowerCase() ?? ''

  if (normalized) {
    if (normalized === 'application/json' || normalized.endsWith('+json')) {
      return 'json'
    }
    if (normalized === 'application/xml' || normalized === 'text/xml' || normalized.endsWith('+xml')) {
      return 'xml'
    }
    if (normalized === 'text/html' || normalized === 'application/xhtml+xml') {
      return 'html'
    }
    if (
      normalized === 'application/javascript'
      || normalized === 'text/javascript'
      || normalized === 'application/ecmascript'
      || normalized === 'text/ecmascript'
    ) {
      return 'javascript'
    }
    if (normalized === 'text/css') {
      return 'css'
    }
    return 'plaintext'
  }

  const trimmed = value.trim()
  if (trimmed) {
    try {
      JSON.parse(trimmed)
      return 'json'
    } catch {
      // Continue with conservative markup inference.
    }
  }
  if (/^<!doctype\s+html\b/i.test(trimmed) || /^<html\b/i.test(trimmed)) {
    return 'html'
  }
  if (/^<\?xml\b/i.test(trimmed) || /^<[A-Za-z_][\w:.-]*(?:\s|\/?>)/.test(trimmed)) {
    return 'xml'
  }
  return 'plaintext'
}

export function bodyEditorPolicy(
  value: string,
  contentType: string | null | undefined,
  readOnly: boolean,
  forcePlaintext = false
): BodyEditorPolicy {
  const large = new Blob([value]).size > LARGE_BODY_THRESHOLD
  const detectedLanguage = forcePlaintext ? 'plaintext' : detectBodyLanguage(contentType, value)
  const language = large ? 'plaintext' : detectedLanguage
  let displayValue = value

  if (readOnly && !large && language === 'json' && value.trim()) {
    try {
      displayValue = JSON.stringify(JSON.parse(value), null, 2)
    } catch {
      displayValue = value
    }
  }

  return {
    value: displayValue,
    language,
    large,
    diagnostics: !large
  }
}
