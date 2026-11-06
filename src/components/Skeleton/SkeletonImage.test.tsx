import { fireEvent, render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { SkeletonImage } from './SkeletonImage';

describe('SkeletonImage', () => {
  it('stays hidden until loaded, and again when the source changes', () => {
    const { container, rerender } = render(<SkeletonImage src="a.jpg" />);
    const img = container.querySelector('img')!;
    expect(img).toHaveClass('img-loading');
    fireEvent.load(img);
    expect(img).toHaveClass('img-loaded');
    rerender(<SkeletonImage src="b.jpg" />);
    expect(img).toHaveClass('img-loading');
    fireEvent.error(img);
    expect(img).not.toHaveClass('img-loading');
  });
});
