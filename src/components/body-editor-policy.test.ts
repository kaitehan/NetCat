import { describe, expect, it } from 'vitest'
import { bodyEditorPolicy, detectBodyLanguage } from './body-editor-policy'

describe('detectBodyLanguage', () => {
  it.each([
    ['application/json; charset=utf-8', 'json'],
    ['application/problem+json', 'json'],
    ['application/xml', 'xml'],
    ['image/svg+xml', 'xml'],
    ['text/html', 'html'],
    ['application/javascript', 'javascript'],
    ['text/css', 'css'],
    ['application/octet-stream', 'plaintext']
  ])('maps %s to %s', (contentType, language) => {
    expect(detectBodyLanguage(contentType, '')).toBe(language)
  })

  it('infers JSON, XML and HTML only when Content-Type is absent', () => {
    expect(detectBodyLanguage(null, '{"ok":true}')).toBe('json')
    expect(detectBodyLanguage(undefined, '<?xml version="1.0"?><ok />')).toBe('xml')
    expect(detectBodyLanguage('', '<!doctype html><html></html>')).toBe('html')
    expect(detectBodyLanguage(null, '{not json}')).toBe('plaintext')
  })
})

describe('bodyEditorPolicy', () => {
  it('pretty prints valid read-only JSON without changing invalid JSON', () => {
    expect(bodyEditorPolicy('{"ok":true}', 'application/json', true).value)
      .toBe('{\n  "ok": true\n}')
    expect(bodyEditorPolicy('{bad', 'application/json', true).value).toBe('{bad')
  })

  it('does not rewrite editable JSON', () => {
    expect(bodyEditorPolicy('{"ok":true}', 'application/json', false).value)
      .toBe('{"ok":true}')
  })

  it('uses plaintext and disables validation for bodies over 1 MB and Hex', () => {
    const large = bodyEditorPolicy('x'.repeat(1024 * 1024 + 1), 'application/json', true)
    expect(large).toMatchObject({ language: 'plaintext', large: true, diagnostics: false })
    expect(bodyEditorPolicy('00000000  1f 8b', null, true, true).language).toBe('plaintext')
  })
})
