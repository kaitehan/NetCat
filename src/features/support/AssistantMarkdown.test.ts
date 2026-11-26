import { render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import AssistantMarkdown from './AssistantMarkdown.vue'

describe('AssistantMarkdown', () => {
  it('renders markdown and removes dangerous HTML', () => {
    const { container } = render(AssistantMarkdown, {
      props: {
        content: '# 结论\n\n- 第一项\n- 第二项\n\n| 方法 | 状态 |\n| --- | --- |\n| GET | 200 |\n\n`code`\n\n<script>alert(1)</script>'
      }
    })

    expect(screen.getByRole('heading', { name: '结论' })).toBeVisible()
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    expect(screen.getByRole('table')).toBeVisible()
    expect(container.querySelector('code')).toHaveTextContent('code')
    expect(container.querySelector('script')).toBeNull()
  })

  it('adds safe attributes to external links', () => {
    render(AssistantMarkdown, {
      props: { content: '[查看文档](https://example.com/docs)' }
    })

    expect(screen.getByRole('link', { name: '查看文档' })).toHaveAttribute('target', '_blank')
    expect(screen.getByRole('link', { name: '查看文档' })).toHaveAttribute('rel', 'noopener noreferrer')
  })
})
