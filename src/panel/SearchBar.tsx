import { forwardRef } from "react";

interface Props {
  value: string;
  onChange: (v: string) => void;
  searching: boolean;
}

const SearchBar = forwardRef<HTMLInputElement, Props>(function SearchBar(
  { value, onChange, searching },
  ref,
) {
  return (
    <div className="relative px-3 py-2 bg-stone-50">
      <input
        ref={ref}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="搜索你的剪切板..."
        className="w-full px-3 py-2 text-sm text-stone-900 bg-stone-50 border border-stone-200 rounded-md outline-none focus:border-stone-400 placeholder:text-stone-400"
        autoFocus
        spellCheck={false}
      />
      {searching && (
        <span className="absolute right-6 top-1/2 -translate-y-1/2 text-[10px] text-stone-400">
          搜索中…
        </span>
      )}
      {value && !searching && (
        <button
          onClick={() => onChange("")}
          className="absolute right-5 top-1/2 -translate-y-1/2 w-4 h-4 rounded-full bg-stone-300 hover:bg-stone-400 text-white flex items-center justify-center"
          tabIndex={-1}
          title="清除搜索"
          aria-label="清除"
        >
          <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
            <path d="M1.5 1.5L6.5 6.5M6.5 1.5L1.5 6.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </div>
  );
});

export default SearchBar;
