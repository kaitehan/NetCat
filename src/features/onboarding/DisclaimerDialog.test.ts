import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import DisclaimerDialog from './DisclaimerDialog.vue'

describe('DisclaimerDialog', () => {
  it('requires explicit authorization acknowledgement', async () => {
    const { emitted } = render(DisclaimerDialog, { props: { open: true } })
    const continueButton = screen.getByRole('button', { name: '进入 NetCapture' })
    expect(continueButton).toBeDisabled()

    await fireEvent.click(screen.getByRole('checkbox'))
    expect(continueButton).toBeEnabled()
    await fireEvent.click(continueButton)
    expect(emitted().accept).toHaveLength(1)
  })
})
