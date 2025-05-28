#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define KB (1024)

int main(int argc, char *argv[]) {
    const char *filename = "testfile";
    FILE *fp;

    // 创建指定大小的文件
    long x = KB; // 示例: 1KB
    fp = fopen(filename, "wb");
    if (fp == NULL) {
        perror("Failed to create file");
        return 1;
    }

    // 填充文件内容
    char buffer[KB];
    memset(buffer, 'A', sizeof(buffer));
    for (long i = 0; i < x / KB; i++) {
        if (fwrite(buffer, 1, KB, fp) != KB) {
            perror("Failed to write to file");
            fclose(fp);
            return 1;
        }
    }

    // 获取初始文件大小
    if (fseek(fp, 0, SEEK_END) != 0) {
        perror("fseek failed");
        fclose(fp);
        return 1;
    }
    long initial_size = ftell(fp);
    printf("初始文件大小: %ld 字节\n", initial_size);

    // 在文件末尾后1KB处写入
    if (fseek(fp, initial_size + KB, SEEK_SET) != 0) {
        perror("fseek failed");
        fclose(fp);
        return 1;
    }

    const char *test_data = "TEST DATA";
    if (fwrite(test_data, 1, strlen(test_data), fp) != strlen(test_data)) {
        perror("Failed to write test data");
        fclose(fp);
        return 1;
    }

    // 关闭文件后重新打开以获取准确大小
    fclose(fp);

    // 验证文件大小
    fp = fopen(filename, "rb");
    if (fp == NULL) {
        perror("Failed to reopen file");
        return 1;
    }

    if (fseek(fp, 0, SEEK_END) != 0) {
        perror("fseek failed");
        fclose(fp);
        return 1;
    }
    long final_size = ftell(fp);
    printf("最终文件大小: %ld 字节\n", final_size);

    // 验证
    long expected_size = initial_size + KB + strlen(test_data);
    if (final_size == expected_size) {
        printf("验证通过: 文件大小符合预期\n");
    } else {
        printf("验证失败: 期望大小为 %ld 字节，但实际为 %ld 字节\n", expected_size, final_size);
        fclose(fp);
        return 1;
    }

    fclose(fp);
    return 0;
}
    