import { mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import BodyEditor from './BodyEditor.vue'

const loader = vi.hoisted(() => ({
  loadMonaco: vi.fn()
}))

vi.mock('./monaco-loader', () => loader)

interface FakeMonaco {
  api: Record<string, unknown>
  fireContentChange: () => void
  model: {
    getValue: () => string
    setValue: (value: string) => void
    dispose: () => void
  }
  editor: {
    focus: () => void
    updateOptions: (options: unknown) => void
    layout: () => void
    dispose: () => void
  }
  subscription: { dispose: () => void }
  setModelLanguage: (model: unknown, language: string) => void
}

function fakeMonaco(initialValue = ''): FakeMonaco {
  let value = initialValue
  let contentListener: () => void = () => undefined
  const subscription = { dispose: vi.fn() }
  const model = {
    getValue: vi.fn(() => value),
    setValue: vi.fn((nextValue: string) => {
      value = nextValue
      contentListener()
    }),
    onDidChangeContent: vi.fn((listener: () => void) => {
      contentListener = listener
      return subscription
    }),
    dispose: vi.fn()
  }
  const editor = {
    focus: vi.fn(),
    updateOptions: vi.fn(),
    layout: vi.fn(),
    dispose: vi.fn()
  }
  const setModelLanguage = vi.fn()
  return {
    api: {
      editor: {
        defineTheme: vi.fn(),
        createModel: vi.fn((nextValue: string) => {
          value = nextValue
          return model
        }),
        create: vi.fn(() => editor),
        setModelLanguage
      }
    },
    fireContentChange: () => contentListener(),
    model,
    editor,
    subscription,
    setModelLanguage
  }
}

beforeEach(() => {
  loader.loadMonaco.mockReset()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('BodyEditor', () => {
  it('creates a read-only editor with stable options and updates value and language', async () => {
    const monaco = fakeMonaco('first')
    loader.loadMonaco.mockResolvedValue(monaco.api)
    const wrapper = mount(BodyEditor, {
      props: {
        modelValue: 'first',
        language: 'json',
        readOnly: true,
        ariaLabel: '请求正文'
      }
    })
    await vi.waitFor(() => expect(
      (monaco.api.editor as Record<string, ReturnType<typeof vi.fn>>).create
    ).toHaveBeenCalled())

    expect((monaco.api.editor as Record<string, ReturnType<typeof vi.fn>>).create)
      .toHaveBeenCalledWith(expect.any(HTMLElement), expect.objectContaining({
        readOnly: true,
        lineNumbers: 'on',
        folding: true,
        wordWrap: 'on',
        automaticLayout: true,
        minimap: { enabled: false }
      }))

    await wrapper.setProps({ modelValue: 'second', language: 'xml' })
    expect(monaco.model.setValue).toHaveBeenCalledWith('second')
    expect(monaco.setModelLanguage).toHaveBeenCalledWith(monaco.model, 'xml')
  })

  it('emits edits, exposes focus, and releases resources on unmount', async () => {
    const monaco = fakeMonaco('before')
    loader.loadMonaco.mockResolvedValue(monaco.api)
    const disconnect = vi.fn()
    const observe = vi.fn()
    vi.stubGlobal('ResizeObserver', class {
      observe() {
        observe()
      }

      disconnect() {
        disconnect()
      }
    })
    const wrapper = mount(BodyEditor, {
      props: {
        modelValue: 'before',
        language: 'plaintext',
        ariaLabel: '响应正文'
      }
    })
    await vi.waitFor(() => expect(
      (monaco.api.editor as Record<string, ReturnType<typeof vi.fn>>).create
    ).toHaveBeenCalled())

    monaco.model.setValue('after')
    monaco.fireContentChange()
    expect(wrapper.emitted('update:modelValue')).toContainEqual(['after'])
    ;(wrapper.vm as unknown as { focus: () => void }).focus()
    expect(monaco.editor.focus).toHaveBeenCalled()

    wrapper.unmount()
    expect(disconnect).toHaveBeenCalled()
    expect(monaco.subscription.dispose).toHaveBeenCalled()
    expect(monaco.editor.dispose).toHaveBeenCalled()
    expect(monaco.model.dispose).toHaveBeenCalled()
  })

  it('falls back to an editable textarea when Monaco fails to load', async () => {
    loader.loadMonaco.mockRejectedValue(new Error('load failed'))
    const wrapper = mount(BodyEditor, {
      props: {
        modelValue: '{"raw":true}',
        language: 'json',
        ariaLabel: '响应正文'
      }
    })
    await vi.waitFor(() => expect(wrapper.find('textarea').exists()).toBe(true))

    await wrapper.find('textarea').setValue('{"raw":false}')
    expect(wrapper.emitted('update:modelValue')).toContainEqual(['{"raw":false}'])
  })

  it('falls back to a read-only pre and honors focus requested while loading', async () => {
    let resolveMonaco: ((value: Record<string, unknown>) => void) | undefined
    const monaco = fakeMonaco('body')
    loader.loadMonaco.mockReturnValue(new Promise((resolve) => {
      resolveMonaco = resolve
    }))
    const wrapper = mount(BodyEditor, {
      props: {
        modelValue: 'body',
        readOnly: true,
        ariaLabel: '响应正文'
      }
    })
    ;(wrapper.vm as unknown as { focus: () => void }).focus()
    resolveMonaco?.(monaco.api)
    await nextTick()
    await vi.waitFor(() => expect(monaco.editor.focus).toHaveBeenCalled())
    wrapper.unmount()

    loader.loadMonaco.mockRejectedValue(new Error('load failed'))
    const fallback = mount(BodyEditor, {
      props: {
        modelValue: 'body',
        readOnly: true,
        ariaLabel: '响应正文'
      }
    })
    await vi.waitFor(() => expect(fallback.find('pre.body-editor-fallback').text()).toBe('body'))
  })
})
