import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import DisclaimerDialog from './DisclaimerDialog.vue'

describe('DisclaimerDialog', () => {
  it('explains proxy scripting and the unsupported certificate bypass boundary', () => {
    render(DisclaimerDialog, { props: { open: true } })

    expect(screen.getByRole('dialog')).toHaveTextContent('本工具不提供证书固定绕过或代码注入能力。')
    expect(screen.getByRole('dialog')).toHaveTextContent('QuickJS 代理脚本仅用于自有或明确授权的网络流量调试')
    expect(screen.getByRole('dialog')).not.toHaveTextContent('Frida')
    expect(screen.getByRole('dialog')).not.toHaveTextContent('SSL Pinning')
  })

  it('requires explicit authorization acknowledgement', async () => {
    const { emitted } = render(DisclaimerDialog, { props: { open: true } })
    const continueButton = screen.getByRole('button', { name: '进入 NetCat' })
    expect(continueButton).toBeDisabled()

    await fireEvent.click(screen.getByRole('checkbox'))
    expect(continueButton).toBeEnabled()
    await fireEvent.click(continueButton)
    expect(emitted().accept).toHaveLength(1)
  })
})
