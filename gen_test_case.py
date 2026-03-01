import random
import string

def generate_random_line(length=10):
    """生成一行随机ASCII字符"""
    return ''.join(random.choices(string.ascii_letters + string.digits, k=length))

def generate_paragraph(lines=5, line_length=10):
    """生成一个段落"""
    paragraph = []
    for _ in range(lines):
        paragraph.append(generate_random_line(line_length))
    return '\n'.join(paragraph)

def generate_text(paragraphs=5, lines_per_paragraph=10, line_length=10):
    """生成完整的文本"""
    text = []
    for i in range(paragraphs):
        text.append(generate_paragraph(lines_per_paragraph, line_length))
    return '\n\n'.join(text)

def main():
    # 生成5段，每段5行，每行10个字符
    random_text = generate_text()
    print(random_text)

if __name__ == "__main__":
    main()
