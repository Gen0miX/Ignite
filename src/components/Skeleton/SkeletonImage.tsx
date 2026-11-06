import { useCallback, useState, type ImgHTMLAttributes } from 'react';

/**
 * `<img>` hidden until it has loaded; its container (class `img-host`, see
 * global.css) pulses like a skeleton meanwhile. A failed image stops the
 * pulse too, the caller's `onError` decides what to show.
 */
export function SkeletonImage({
  className,
  onLoad,
  onError,
  src,
  alt = '',
  ...props
}: ImgHTMLAttributes<HTMLImageElement>) {
  const [done, setDone] = useState<string | undefined>(undefined);
  const loading = done !== src;
  // Already in the cache: `complete` before React sees the load event.
  const ref = useCallback(
    (el: HTMLImageElement | null) => {
      if (el?.complete && el.naturalWidth > 0) setDone(src);
    },
    [src],
  );
  return (
    <img
      ref={ref}
      src={src}
      alt={alt}
      className={`${className ?? ''} ${loading ? 'img-loading' : 'img-loaded'}`}
      onLoad={(e) => {
        setDone(src);
        onLoad?.(e);
      }}
      onError={(e) => {
        setDone(src);
        onError?.(e);
      }}
      {...props}
    />
  );
}
